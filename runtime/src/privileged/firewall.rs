//! Managed Windows Firewall inbound-allow rule for the Engine exe (KOS-269).
//!
//! Windows Defender Firewall keys its "Allow access" prompt — and the rules
//! created from it — by exe path. The Engine runs from the versioned install
//! dir (`%LOCALAPPDATA%\Mundus\Engine\versions\<v>\mundus-engine.exe`), so
//! every update produced a new prompt. The privileged service keeps ONE
//! inbound allow rule pointing at the *current* Engine exe instead; the
//! Engine asks for a refresh at each boot via the `ensure_engine_allow` pipe
//! request.
//!
//! Layout of this module:
//!   * the rule spec and the ensure/remove orchestration are pure functions
//!     over the [`FirewallPolicy`] trait — unit tests run them against a fake
//!     and never touch the real firewall;
//!   * `policy2` (Windows only) is the `INetFwPolicy2` COM implementation;
//!   * [`validated_engine_image`] is the service-side security check: the
//!     rule's program path is derived from the pipe *client's* process image
//!     (never from request parameters) and must be a `mundus-engine.exe`
//!     inside the calling user's own versioned install dir.

use std::path::{Component, Path, PathBuf};

use crate::brand;
use crate::engine_versions::engine_root_of_exe;

#[cfg(windows)]
mod policy2;

/// Stable firewall rule name. Renaming orphans the existing rule, so this
/// string ships once and never changes.
pub const RULE_NAME: &str = "Mundus Engine — LAN sync";
/// Rule `Grouping` — groups the entry under the product in wf.msc.
pub const RULE_GROUPING: &str = "Mundus Engine";
/// Human-readable `Description` on the rule (English is acceptable for
/// firewall internals; the user-facing explanation lives in the Manager).
pub const RULE_DESCRIPTION: &str =
    "Inbound LAN sync for Mundus Engine. Maintained by the Mundus privileged service \
     so the allowed program path follows Engine updates.";

/// `INetFwRule.Protocol` value `NET_FW_IP_PROTOCOL_ANY` (256): one rule
/// covers both TCP and UDP — the same grant the Windows "Allow access"
/// prompt produces — instead of a pair of per-protocol rules.
pub const PROTOCOL_ANY: i32 = 256;

/// `INetFwRule.Profiles` bitmask `NET_FW_PROFILE2_DOMAIN | PRIVATE` (1|2).
/// LAN sync pairs devices on home/work networks; Public profiles stay
/// closed — that is the profile class where an open inbound port is most
/// likely to be hostile (cafés, airports).
pub const PROFILES_DOMAIN_PRIVATE: i32 = 3;

/// Everything needed to (re)create the managed inbound-allow rule.
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
}

/// The managed rule, with `program` filled in by the caller.
pub fn engine_allow_spec(program: &Path) -> RuleSpec {
    RuleSpec {
        name: RULE_NAME,
        grouping: RULE_GROUPING,
        description: RULE_DESCRIPTION,
        program: program.to_string_lossy().into_owned(),
        protocol: PROTOCOL_ANY,
        profiles: PROFILES_DOMAIN_PRIVATE,
    }
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
    pub action_allow: bool,
}

impl ObservedRule {
    /// Does the live rule already say what the spec wants? `program`
    /// compares case-insensitively (Windows paths).
    pub fn matches(&self, spec: &RuleSpec) -> bool {
        self.enabled
            && self.direction_inbound
            && self.action_allow
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

/// Idempotent refresh: create/update the managed rule only when the stored
/// rule does not already match the spec.
pub fn ensure_engine_rule(
    policy: &mut impl FirewallPolicy,
    program: &Path,
) -> Result<EnsureOutcome, String> {
    let spec = engine_allow_spec(program);
    match policy.rule(spec.name)? {
        Some(observed) if observed.matches(&spec) => Ok(EnsureOutcome::Unchanged),
        _ => {
            policy.put(&spec)?;
            Ok(EnsureOutcome::Updated)
        }
    }
}

/// Remove the managed rule (used by `privileged uninstall`).
pub fn remove_engine_rule(policy: &mut impl FirewallPolicy) -> Result<(), String> {
    policy.remove(RULE_NAME)
}

/// `<profile>\AppData\Local\<Brand>\Engine` — the only install root whose
/// `versions\<semver>\mundus-engine.exe` the service may open the firewall
/// for. `profile` is the *pipe client's* profile dir resolved under
/// impersonation, so the path is always scoped to the calling user.
pub fn engine_install_root(profile_dir: &Path) -> PathBuf {
    profile_dir
        .join("AppData")
        .join("Local")
        .join(brand::LOCAL_DIR_NAME)
        .join(brand::ENGINE_DIR_NAME)
}

/// Case-insensitive, component-wise path equality (Windows semantics):
/// `\\?\` verbatim prefixes are never produced here — callers reject them
/// first — so component comparison is exact enough for our fixed shape.
fn paths_equal_ci(a: &Path, b: &Path) -> bool {
    let ac: Vec<String> = a
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let bc: Vec<String> = b
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    ac == bc
}

/// Validate the pipe client's process image as the program a firewall rule
/// may point at. Accepted: `<profile>\AppData\Local\<Brand>\Engine\
/// versions\<semver>\mundus-engine.exe` — nothing else. The service runs as
/// SYSTEM, so a client-supplied or loosely-checked path would be a primitive
/// for opening the firewall to an arbitrary binary.
pub fn validated_engine_image(profile_dir: &Path, image_path: &Path) -> Result<PathBuf, String> {
    for component in image_path.components() {
        // UNC (`\\host\share\…`) and verbatim (`\\?\…`, `\\.\…`) paths are
        // refused outright: the rule must scope to a local drive-letter
        // path. `VerbatimDisk` is still rejected — it is the same path
        // spelled `\\?\C:\…` and we never produce that form ourselves.
        if let Component::Prefix(prefix) = component {
            if !matches!(prefix.kind(), std::path::Prefix::Disk(_)) {
                return Err(format!(
                    "refusing non-local image path {}",
                    image_path.display()
                ));
            }
        }
        if matches!(component, Component::ParentDir) {
            return Err(format!(
                "refusing traversal in image path {}",
                image_path.display()
            ));
        }
    }
    // `components()` silently normalizes `.` away — scan the raw segments so
    // `a\.\b` is rejected too: the rule name is the only accepted shape.
    if image_path
        .to_string_lossy()
        .split(['\\', '/'])
        .any(|seg| seg == "." || seg == "..")
    {
        return Err(format!(
            "refusing traversal in image path {}",
            image_path.display()
        ));
    }
    // `engine_root_of_exe` also enforces the `mundus-engine.exe` name and
    // the `versions/<semver>` parent shape.
    let root = engine_root_of_exe(image_path)
        .ok_or_else(|| format!("{} is not a versioned Engine install", image_path.display()))?;
    let expected = engine_install_root(profile_dir);
    if !paths_equal_ci(&root, &expected) {
        return Err(format!(
            "{} is outside the caller's Engine install root {}",
            image_path.display(),
            expected.display()
        ));
    }
    Ok(image_path.to_path_buf())
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
    ensure_engine_rule(&mut policy, &exe)
}

/// `privileged uninstall`: drop the managed rule. Best-effort callers log
/// the error; a missing rule is already `Ok(())`.
#[cfg(windows)]
pub fn uninstall_cleanup() -> Result<(), String> {
    let mut policy = policy2::NetFwPolicy2::open()?;
    remove_engine_rule(&mut policy)
}

#[cfg(test)]
#[path = "firewall_tests.rs"]
mod tests;
