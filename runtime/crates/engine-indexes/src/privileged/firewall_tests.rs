//! Unit tests for the pure firewall-rule logic. These never touch the real
//! firewall — `FakePolicy` is an in-memory [`FirewallPolicy`].

use std::collections::HashMap;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

use super::*;

#[derive(Default)]
struct FakePolicy {
    rules: HashMap<String, ObservedRule>,
    /// Full specs as last written by `put` — lets tests assert on what the
    /// real COM layer would have received.
    written: Vec<RuleSpec>,
    removed: Vec<String>,
    fail_rule: bool,
}

impl FirewallPolicy for FakePolicy {
    fn rule(&self, name: &str) -> Result<Option<ObservedRule>, String> {
        if self.fail_rule {
            return Err("lookup failed".into());
        }
        Ok(self.rules.get(name).cloned())
    }

    fn put(&mut self, spec: &RuleSpec) -> Result<(), String> {
        self.written.push(spec.clone());
        self.rules.insert(
            spec.name.to_string(),
            ObservedRule {
                program: Some(spec.program.clone()),
                enabled: true,
                protocol: spec.protocol,
                profiles: spec.profiles,
                direction_inbound: true,
                action: spec.action,
            },
        );
        Ok(())
    }

    fn remove(&mut self, name: &str) -> Result<(), String> {
        self.removed.push(name.to_string());
        self.rules.remove(name);
        Ok(())
    }
}

const EXE: &str = r"C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.3\mundus-engine.exe";

fn allow_spec(path: &str) -> RuleSpec {
    engine_rule_specs(Path::new(path))[0].clone()
}

#[test]
fn rule_specs_are_stable_inbound_allow_plus_public_block() {
    let [allow, block] = engine_rule_specs(Path::new(EXE));

    assert_eq!(allow.name, RULE_NAME);
    assert_eq!(allow.grouping, RULE_GROUPING);
    assert_eq!(allow.program, EXE);
    // Any-protocol single rule covering TCP+UDP, Domain+Private profiles only.
    assert_eq!(allow.protocol, PROTOCOL_ANY);
    assert_eq!(allow.profiles, PROFILES_DOMAIN_PRIVATE);
    assert_eq!(allow.action, RuleAction::Allow);

    // The Public-profile sibling suppresses the "allow access" prompt on
    // public networks without opening the port (KOS-269 round 1).
    assert_eq!(block.name, BLOCK_RULE_NAME);
    assert_eq!(block.grouping, RULE_GROUPING);
    assert_eq!(block.program, EXE);
    assert_eq!(block.protocol, PROTOCOL_ANY);
    assert_eq!(block.profiles, PROFILES_PUBLIC);
    assert_eq!(block.action, RuleAction::Block);
}

#[test]
fn ensure_writes_both_rules_once_then_is_idempotent() {
    let mut fw = FakePolicy::default();
    let exe = Path::new(EXE);

    assert_eq!(
        ensure_engine_rules(&mut fw, exe),
        Ok(EnsureOutcome::Updated)
    );
    assert_eq!(fw.written.len(), 2);

    // Second call: both rules already point at this exe — no writes.
    assert_eq!(
        ensure_engine_rules(&mut fw, exe),
        Ok(EnsureOutcome::Unchanged)
    );
    assert_eq!(fw.written.len(), 2);
}

#[test]
fn ensure_repoints_both_rules_after_engine_update() {
    let mut fw = FakePolicy::default();
    let old =
        Path::new(r"C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.3\mundus-engine.exe");
    let new =
        Path::new(r"C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.4\mundus-engine.exe");

    assert_eq!(
        ensure_engine_rules(&mut fw, old),
        Ok(EnsureOutcome::Updated)
    );
    assert_eq!(
        ensure_engine_rules(&mut fw, new),
        Ok(EnsureOutcome::Updated)
    );
    // Exactly two rewrites, both pointed at the new exe.
    let repointed: Vec<_> = fw.written[2..].iter().collect();
    assert_eq!(repointed.len(), 2);
    assert!(repointed.iter().all(|s| s.program == new.to_string_lossy()));
}

#[test]
fn ensure_replaces_stale_or_disabled_rule() {
    let mut fw = FakePolicy::default();
    let spec = allow_spec(EXE);

    for stale in [
        ObservedRule {
            program: Some(spec.program.clone()),
            enabled: false, // user disabled it — we re-assert our spec
            protocol: spec.protocol,
            profiles: spec.profiles,
            direction_inbound: true,
            action: RuleAction::Allow,
        },
        ObservedRule {
            program: Some(spec.program.clone()),
            enabled: true,
            protocol: spec.protocol,
            profiles: 7, // drifted onto Public
            direction_inbound: true,
            action: RuleAction::Allow,
        },
        ObservedRule {
            program: Some(spec.program.clone()),
            enabled: true,
            protocol: spec.protocol,
            profiles: spec.profiles,
            direction_inbound: false, // outbound rule squatting on the name
            action: RuleAction::Allow,
        },
        ObservedRule {
            program: Some(spec.program.clone()),
            enabled: true,
            protocol: spec.protocol,
            profiles: spec.profiles,
            direction_inbound: true,
            action: RuleAction::Block, // wrong action on the allow name
        },
    ] {
        fw.rules.insert(spec.name.to_string(), stale);
        assert_eq!(
            ensure_engine_rules(&mut fw, Path::new(&spec.program)),
            Ok(EnsureOutcome::Updated)
        );
        fw.written.clear();
    }
}

#[test]
fn ensure_propagates_lookup_errors() {
    let mut fw = FakePolicy {
        fail_rule: true,
        ..FakePolicy::default()
    };
    let err = ensure_engine_rules(&mut fw, Path::new("x")).unwrap_err();
    assert_eq!(err, "lookup failed");
}

#[test]
fn remove_targets_only_our_rule_names() {
    let mut fw = FakePolicy::default();
    fw.rules
        .insert(RULE_NAME.to_string(), ObservedRule::default());
    fw.rules
        .insert(BLOCK_RULE_NAME.to_string(), ObservedRule::default());
    fw.rules
        .insert("Some Other Rule".to_string(), ObservedRule::default());

    remove_engine_rules(&mut fw).unwrap();
    assert_eq!(
        fw.removed,
        vec![RULE_NAME.to_string(), BLOCK_RULE_NAME.to_string()]
    );
    assert!(fw.rules.contains_key("Some Other Rule"));
    assert!(!fw.rules.contains_key(RULE_NAME));
    assert!(!fw.rules.contains_key(BLOCK_RULE_NAME));
}

// ------------------------- path validation -------------------------
// These fixtures are Windows path forms: drive letters, `\` separators and
// case-insensitive comparison — the shapes a real pipe client image takes.
// On unix a `C:\` literal is a single component, so they can only run there.

#[cfg(windows)]
fn profile() -> PathBuf {
    PathBuf::from(r"C:\Users\anna")
}

#[cfg(windows)]
fn good_image() -> PathBuf {
    profile().join(r"AppData\Local\Mundus\Engine\versions\0.10.3\mundus-engine.exe")
}

#[cfg(windows)]
#[test]
fn versioned_engine_of_the_calling_user_is_accepted() {
    let exe = validated_engine_image(&profile(), &good_image()).unwrap();
    assert_eq!(exe, good_image());
    // Case-insensitive filesystem semantics.
    let upper = PathBuf::from(
        r"c:\users\ANNA\appdata\local\mundus\engine\versions\0.10.3\MUNDUS-ENGINE.EXE",
    );
    assert!(validated_engine_image(&profile(), &upper).is_ok());
}

#[cfg(windows)]
#[test]
fn other_users_install_is_rejected() {
    let image = PathBuf::from(
        r"C:\Users\bobby\AppData\Local\Mundus\Engine\versions\0.10.3\mundus-engine.exe",
    );
    assert!(validated_engine_image(&profile(), &image).is_err());
}

#[cfg(windows)]
#[test]
fn paths_outside_versions_semver_are_rejected() {
    for image in [
        r"C:\Users\anna\AppData\Local\Mundus\Engine\mundus-engine.exe",
        r"C:\Users\anna\AppData\Local\Mundus\Engine\versions\not-a-version\mundus-engine.exe",
        r"C:\Users\anna\AppData\Local\Mundus\versions\0.10.3\mundus-engine.exe",
        r"C:\Program Files\Mundus\Service\versions\0.10.3\mundus-engine.exe",
        r"C:\mundus-engine.exe",
    ] {
        assert!(
            validated_engine_image(&profile(), Path::new(image)).is_err(),
            "accepted {image}"
        );
    }
}

#[cfg(windows)]
#[test]
fn foreign_exe_names_are_rejected() {
    for image in [
        r"C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.3\evil.exe",
        r"C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.3\mundus-engine.exe.bak",
        r"C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.3\mundus-privileged-service.exe",
    ] {
        assert!(
            validated_engine_image(&profile(), Path::new(image)).is_err(),
            "accepted {image}"
        );
    }
}

#[cfg(windows)]
#[test]
fn traversal_and_unc_paths_are_rejected() {
    for image in [
        // `..` inside the install root.
        r"C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.3\..\mundus-engine.exe",
        r"C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.3\.\mundus-engine.exe",
        // UNC forms — must never reach the rule.
        r"\\server\share\anna\AppData\Local\Mundus\Engine\versions\0.10.3\mundus-engine.exe",
        r"\\?\C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.3\mundus-engine.exe",
        r"\\.\C:\Users\anna\AppData\Local\Mundus\Engine\versions\0.10.3\mundus-engine.exe",
    ] {
        assert!(
            validated_engine_image(&profile(), Path::new(image)).is_err(),
            "accepted {image}"
        );
    }
}
