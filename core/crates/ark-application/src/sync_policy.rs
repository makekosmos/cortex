use ark_domain::selective_sync::{FilterV1, SelectiveSyncProfile, SyncMode};
use sha2::{Digest, Sha256};

/// Exact receiver-owned policy identity used to bind sync delivery to the
/// policy that selected its contents.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PolicyIdentity {
    pub revision: i64,
    pub digest: String,
    pub active: bool,
}

impl PolicyIdentity {
    pub fn new(revision: i64, digest: impl Into<String>, active: bool) -> Result<Self, String> {
        let digest = digest.into();
        if revision < 0
            || digest.len() != 64
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid_sync_policy_identity".into());
        }
        Ok(Self {
            revision,
            digest,
            active,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncProfileRule {
    pub resource_kind: String,
    pub resource_id: String,
    pub mode: SyncMode,
    pub filter: FilterV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncProfile {
    pub profile_id: String,
    pub device_id: String,
    pub revision: i64,
    pub name: String,
    pub active: bool,
    pub rules: Vec<SyncProfileRule>,
}

impl SyncProfile {
    pub fn digest(&self) -> String {
        digest_sync_policy(&self.rules)
    }

    pub fn selective_profile(&self) -> SelectiveSyncProfile {
        let mut profile = SelectiveSyncProfile::default();
        for rule in &self.rules {
            profile.set_rule_with_filter(
                &rule.resource_kind,
                &rule.resource_id,
                rule.mode,
                rule.filter.clone(),
            );
        }
        profile
    }
}

/// Receiver-owned policy cached by the sender for one authenticated peer.
/// The receiver's local profile remains authoritative; this copy only controls
/// what this device sends to that peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedPeerSyncPolicy {
    pub peer_device_id: String,
    pub revision: i64,
    pub digest: String,
    pub active: bool,
    pub rules: Vec<SyncProfileRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskMergeSyncPolicyError {
    FullTypeRequired,
    FilterUnsupported,
}

pub fn validate_task_merge_sync_policy(
    profile: &SelectiveSyncProfile,
) -> Result<(), TaskMergeSyncPolicyError> {
    let Some(rule) = profile
        .rules
        .get(&(String::from("type"), String::from("core.task")))
    else {
        return Err(TaskMergeSyncPolicyError::FullTypeRequired);
    };
    if rule.mode != SyncMode::Full {
        return Err(TaskMergeSyncPolicyError::FullTypeRequired);
    }
    if rule.filter.digest() != FilterV1::match_all().digest() {
        return Err(TaskMergeSyncPolicyError::FilterUnsupported);
    }
    Ok(())
}

impl AcceptedPeerSyncPolicy {
    pub fn identity(&self) -> PolicyIdentity {
        PolicyIdentity {
            revision: self.revision,
            digest: self.digest.clone(),
            active: self.active,
        }
    }

    pub fn computed_digest(&self) -> String {
        digest_sync_policy(&self.rules)
    }

    pub fn selective_profile(&self) -> SelectiveSyncProfile {
        let mut profile = SelectiveSyncProfile::default();
        for rule in &self.rules {
            profile.set_rule_with_filter(
                &rule.resource_kind,
                &rule.resource_id,
                rule.mode,
                rule.filter.clone(),
            );
        }
        profile
    }
}

pub fn digest_sync_policy(rules: &[SyncProfileRule]) -> String {
    let mut rules = rules
        .iter()
        .map(|rule| {
            (
                rule.resource_kind.as_str(),
                rule.resource_id.as_str(),
                sync_mode_name(rule.mode),
                rule.filter.canonical_json(),
            )
        })
        .collect::<Vec<_>>();
    rules.sort_unstable();
    let canonical = serde_json::to_vec(&(1u8, rules)).expect("sync policy fields serialize");
    let mut digest = Sha256::new();
    digest.update(b"cortex-sync-policy-v1\0");
    digest.update(canonical);
    format!("{:x}", digest.finalize())
}

pub fn default_sync_policy_identity() -> PolicyIdentity {
    PolicyIdentity {
        revision: 0,
        digest: digest_sync_policy(&[]),
        active: false,
    }
}

pub fn sync_profile_identity(profile: Option<&SyncProfile>) -> PolicyIdentity {
    profile.map_or_else(default_sync_policy_identity, |profile| PolicyIdentity {
        revision: profile.revision,
        digest: profile.digest(),
        active: profile.active,
    })
}

fn sync_mode_name(mode: SyncMode) -> &'static str {
    match mode {
        SyncMode::Full => "full",
        SyncMode::Metadata => "metadata",
        SyncMode::None => "none",
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_task_merge_sync_policy, TaskMergeSyncPolicyError};
    use ark_domain::selective_sync::{FilterV1, SelectiveSyncProfile, SyncMode};

    #[test]
    fn task_merge_requires_full_unfiltered_task_sync() {
        let mut profile = SelectiveSyncProfile::default();
        assert_eq!(
            validate_task_merge_sync_policy(&profile),
            Err(TaskMergeSyncPolicyError::FullTypeRequired)
        );

        profile.set_rule("type", "core.task", SyncMode::Metadata);
        assert_eq!(
            validate_task_merge_sync_policy(&profile),
            Err(TaskMergeSyncPolicyError::FullTypeRequired)
        );

        profile.set_rule_with_filter(
            "type",
            "core.task",
            SyncMode::Full,
            FilterV1::parse(r#"{"version":1,"where":{"eq":{"field":"status","value":"open"}}}"#)
                .expect("valid test filter"),
        );
        assert_eq!(
            validate_task_merge_sync_policy(&profile),
            Err(TaskMergeSyncPolicyError::FilterUnsupported)
        );

        profile.set_rule("type", "core.task", SyncMode::Full);
        assert_eq!(validate_task_merge_sync_policy(&profile), Ok(()));
    }
}
