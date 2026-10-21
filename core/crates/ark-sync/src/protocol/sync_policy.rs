/// Canonical policy rule exchanged only after protocol capability negotiation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncPolicyRuleWire {
    pub resource_kind: String,
    pub resource_id: String,
    pub mode: ark_domain::selective_sync::SyncMode,
    pub filter: serde_json::Value,
}

impl From<&ark_application::sync_policy::SyncProfileRule> for SyncPolicyRuleWire {
    fn from(rule: &ark_application::sync_policy::SyncProfileRule) -> Self {
        Self {
            resource_kind: rule.resource_kind.clone(),
            resource_id: rule.resource_id.clone(),
            mode: rule.mode,
            filter: serde_json::from_str(rule.filter.canonical_json())
                .expect("validated policy filter is valid JSON"),
        }
    }
}

impl TryFrom<SyncPolicyRuleWire> for ark_application::sync_policy::SyncProfileRule {
    type Error = String;

    fn try_from(rule: SyncPolicyRuleWire) -> Result<Self, Self::Error> {
        let filter_json = serde_json::to_string(&rule.filter).map_err(|error| error.to_string())?;
        Ok(Self {
            resource_kind: rule.resource_kind,
            resource_id: rule.resource_id,
            mode: rule.mode,
            filter: ark_domain::selective_sync::FilterV1::parse(&filter_json)?,
        })
    }
}

pub fn sync_policy_message(profile: &ark_application::sync_policy::SyncProfile) -> LanSyncMessage {
    LanSyncMessage::SyncPolicy {
        revision: profile.revision,
        digest: profile.digest(),
        active: profile.active,
        rules: profile.rules.iter().map(SyncPolicyRuleWire::from).collect(),
    }
}

pub fn default_sync_policy_message() -> LanSyncMessage {
    let identity = ark_application::sync_policy::default_sync_policy_identity();
    LanSyncMessage::SyncPolicy {
        revision: identity.revision,
        digest: identity.digest,
        active: identity.active,
        rules: Vec::new(),
    }
}

pub fn sync_policy_identity(
    profile: Option<&ark_application::sync_policy::SyncProfile>,
) -> ark_application::sync_storage::PolicyIdentity {
    ark_application::sync_policy::sync_profile_identity(profile)
}

pub(super) fn default_sync_policy_active() -> bool {
    true
}

pub fn parse_sync_policy_rules(
    rules: Vec<SyncPolicyRuleWire>,
) -> Result<Vec<ark_application::sync_policy::SyncProfileRule>, String> {
    rules.into_iter().map(TryInto::try_into).collect()
}
use serde::{Deserialize, Serialize};

use super::LanSyncMessage;
