//! Managed Windows Firewall inbound-allow rule for the Engine exe (KOS-269).
//!
//! Windows Defender Firewall keys its "Allow access" prompt — and the rules
//! created from it — by exe path. The Engine runs from the versioned install
//! dir (`%LOCALAPPDATA%\Mundus\Engine\versions\<v>\mundus-engine.exe`), so
//! every update produced a new prompt. The privileged service keeps one
//! inbound allow rule (Domain+Private) plus an inbound block rule (Public)
//! pointing at the *current* Engine exe instead; the Engine asks for a
//! refresh at each boot via the `ensure_engine_allow` pipe request.
//!
//! Layout of this module:
//!   * the rule spec and the ensure/remove orchestration are pure functions
//!     over the [`FirewallPolicy`] trait — unit tests run them against a fake
//!     and never touch the real firewall;
//!   * `policy2` (Windows only) is the `INetFwPolicy2` COM implementation;
//!   * `validation::validated_engine_image` is the service-side security
//!     check: the rule's program path is derived from the pipe *client's*
//!     process image (never from request parameters) and must be a
//!     `mundus-engine.exe` inside the calling user's own versioned install
//!     dir.

use std::path::Path;

#[cfg(windows)]
mod policy2;
mod validation;

pub use validation::{engine_install_root, validated_engine_image};

/// Stable firewall rule name. Renaming orphans the existing rule, so this
/// string ships once and never changes.
pub const RULE_NAME: &str = "Mundus Engine — LAN sync";
/// Second managed rule: an explicit inbound *block* on the Public profile.
/// The allow rule covers Domain+Private only, so on a Public network the
/// Engine matched no rule at all and Windows prompted again after every
/// update — the very bug KOS-269 removes. Per [MS-FASP] Appendix B
/// (<https://learn.microsoft.com/openspecs/windows_protocols/ms-fasp/1da2ee70-a6ae-4f76-b08f-fdc25c77d8a0>)
/// the "allow access" notification fires only when no `FW_RULE` object has
/// a matching `wszLocalApplication` — a matching block rule suppresses it
/// too, keeping Public networks closed *and* prompt-free.
pub const BLOCK_RULE_NAME: &str = "Mundus Engine — LAN sync (blocked on public networks)";
/// Rule `Grouping` — groups the entry under the product in wf.msc.
pub const RULE_GROUPING: &str = "Mundus Engine";
/// Human-readable `Description` on the allow rule (English is acceptable
/// for firewall internals; the user-facing explanation lives in the
/// Manager).
pub const RULE_DESCRIPTION: &str =
    "Inbound LAN sync for Mundus Engine. Maintained by the Mundus privileged service \
     so the allowed program path follows Engine updates.";
/// `Description` for the Public-profile block rule.
pub const BLOCK_RULE_DESCRIPTION: &str =
    "Blocks inbound LAN sync for Mundus Engine on public networks. Maintained by the \
     Mundus privileged service; suppresses the firewall prompt after Engine updates.";

/// `INetFwRule.Protocol` value `NET_FW_IP_PROTOCOL_ANY` (256): one rule
/// covers both TCP and UDP — the same grant the Windows "Allow access"
/// prompt produces — instead of a pair of per-protocol rules.
pub const PROTOCOL_ANY: i32 = 256;

/// `INetFwRule.Profiles` bitmask `NET_FW_PROFILE2_DOMAIN | PRIVATE` (1|2).
/// LAN sync pairs devices on home/work networks; Public profiles stay
/// closed — that is the profile class where an open inbound port is most
/// likely to be hostile (cafés, airports).
pub const PROFILES_DOMAIN_PRIVATE: i32 = 3;
/// `NET_FW_PROFILE2_PUBLIC` (4) — the block rule lives here alone.
pub const PROFILES_PUBLIC: i32 = 4;

/// `NET_FW_ACTION` — the only values a managed rule takes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RuleAction {
    #[default]
    Allow,
    Block,
}

/// Everything needed to (re)create one managed inbound rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSpec {
    pub name: &'static str,
    pub grouping: &'static str,
    pub description: &'static str,
    /// Program the rule scopes to — the validated current Engine exe.
    pub program: String,
    /// `INetFwRule.Protocol` (`NET_FW_IP_PROTOCOL_*`).
    pub protocol: i32,
    /// `INetFwRule.Profiles` bitmask (`NET_FW_PROFILE2_*`).
    pub profiles: i32,
    /// `INetFwRule.Action` (`NET_FW_ACTION_*`).
    pub action: RuleAction,
}

fn spec(
    name: &'static str,
    description: &'static str,
    profiles: i32,
    action: RuleAction,
    program: &Path,
) -> RuleSpec {
    RuleSpec {
        name,
        grouping: RULE_GROUPING,
        description,
        program: program.to_string_lossy().into_owned(),
        protocol: PROTOCOL_ANY,
        profiles,
        action,
    }
}

/// The managed rule pair for `program`: inbound allow on Domain+Private,
/// inbound block on Public (see `BLOCK_RULE_NAME`). `ensure` and
/// `uninstall` always handle the pair together.
pub fn engine_rule_specs(program: &Path) -> [RuleSpec; 2] {
    [
        spec(
            RULE_NAME,
            RULE_DESCRIPTION,
            PROFILES_DOMAIN_PRIVATE,
            RuleAction::Allow,
            program,
        ),
        spec(
            BLOCK_RULE_NAME,
            BLOCK_RULE_DESCRIPTION,
            PROFILES_PUBLIC,
            RuleAction::Block,
            program,
        ),
    ]
}

/// What the firewall currently stores under a rule name, reduced to the
/// fields the spec owns. Comparison is a method on the spec side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservedRule {
    pub program: Option<String>,
    pub enabled: bool,
    pub protocol: i32,
    pub profiles: i32,
    pub direction_inbound: bool,
    pub action: RuleAction,
}

impl ObservedRule {
    /// Does the live rule already say what the spec wants? `program`
    /// compares case-insensitively (Windows paths).
    pub fn matches(&self, spec: &RuleSpec) -> bool {
        self.enabled
            && self.direction_inbound
            && self.action == spec.action
            && self.protocol == spec.protocol
            && self.profiles == spec.profiles
            && self
                .program
                .as_deref()
                .is_some_and(|p| p.eq_ignore_ascii_case(&spec.program))
    }
}

/// COM surface the rest of the code depends on. The real implementation is
/// `policy2::NetFwPolicy2`; tests substitute an in-memory fake.
pub trait FirewallPolicy {
    /// The rule stored under `name`, or `None`.
    fn rule(&self, name: &str) -> Result<Option<ObservedRule>, String>;
    /// Create the rule, or overwrite its managed fields when it exists.
    fn put(&mut self, spec: &RuleSpec) -> Result<(), String>;
    /// Delete the rule named `name`; a missing rule is not an error.
    fn remove(&mut self, name: &str) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnsureOutcome {
    /// Rule already pointed at this exe — nothing was written.
    Unchanged,
    /// Rule was created or re-pointed.
    Updated,
}

/// Idempotent refresh of the rule pair: each managed rule is written only
/// when the stored rule does not already match its spec. `Unchanged` means
/// neither rule needed a write.
pub fn ensure_engine_rules(
    policy: &mut impl FirewallPolicy,
    program: &Path,
) -> Result<EnsureOutcome, String> {
    let mut outcome = EnsureOutcome::Unchanged;
    for spec in engine_rule_specs(program) {
        let fresh = matches!(policy.rule(spec.name)?, Some(o) if o.matches(&spec));
        if !fresh {
            policy.put(&spec)?;
            outcome = EnsureOutcome::Updated;
        }
    }
    Ok(outcome)
}

/// Remove both managed rules (used by `privileged uninstall`). A missing
/// rule is not an error.
pub fn remove_engine_rules(policy: &mut impl FirewallPolicy) -> Result<(), String> {
    policy.remove(RULE_NAME)?;
    policy.remove(BLOCK_RULE_NAME)
}

/// Service-side entry point for the `ensure_engine_allow` pipe request:
/// derive the client exe from the pipe (PID → image path, profile dir under
/// impersonation), validate it, then upsert the rule. Returns whether the
/// stored rule actually changed.
#[cfg(windows)]
pub fn ensure_for_pipe_client(
    pipe: windows::Win32::Foundation::HANDLE,
) -> Result<EnsureOutcome, String> {
    let profile = crate::privileged::impersonate::client_profile_dir(pipe)?;
    let image = crate::privileged::impersonate::client_image_path(pipe)?;
    let exe = validated_engine_image(&profile, &image)?;
    let mut policy = policy2::NetFwPolicy2::open()?;
    ensure_engine_rules(&mut policy, &exe)
}

/// `privileged uninstall`: drop both managed rules. Best-effort callers
/// log the error; a missing rule is already `Ok(())`.
#[cfg(windows)]
pub fn uninstall_cleanup() -> Result<(), String> {
    let mut policy = policy2::NetFwPolicy2::open()?;
    remove_engine_rules(&mut policy)
}

#[cfg(test)]
#[path = "firewall_tests.rs"]
mod tests;
