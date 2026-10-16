/// Canonical policy rule exchanged only after protocol capability negotiation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncPolicyRuleWire {
    pub resource_kind: String,
    pub resource_id: String,
    pub mode: crate::data_platform::SyncMode,
    pub filter: serde_json::Value,
}

impl From<&crate::data_platform::SyncProfileRule> for SyncPolicyRuleWire {
    fn from(rule: &crate::data_platform::SyncProfileRule) -> Self {
        Self {
            resource_kind: rule.resource_kind.clone(),
            resource_id: rule.resource_id.clone(),
            mode: rule.mode,
            filter: serde_json::from_str(rule.filter.canonical_json())
                .expect("validated policy filter is valid JSON"),
        }
    }
}

impl TryFrom<SyncPolicyRuleWire> for crate::data_platform::SyncProfileRule {
    type Error = String;

    fn try_from(rule: SyncPolicyRuleWire) -> Result<Self, Self::Error> {
        let filter_json = serde_json::to_string(&rule.filter).map_err(|error| error.to_string())?;
        Ok(Self {
            resource_kind: rule.resource_kind,
            resource_id: rule.resource_id,
            mode: rule.mode,
            filter: crate::data_platform::FilterV1::parse(&filter_json)?,
        })
    }
}

pub fn sync_policy_message(profile: &crate::data_platform::SyncProfile) -> LanSyncMessage {
    LanSyncMessage::SyncPolicy {
        revision: profile.revision,
        digest: profile.digest(),
        active: profile.active,
        rules: profile.rules.iter().map(SyncPolicyRuleWire::from).collect(),
    }
}

pub fn sync_policy_identity(
    profile: Option<&crate::data_platform::SyncProfile>,
) -> crate::application::sync_storage::PolicyIdentity {
    profile.map_or_else(
        crate::data_platform::default_sync_policy_identity,
        |profile| crate::application::sync_storage::PolicyIdentity {
            revision: profile.revision,
            digest: profile.digest(),
            active: profile.active,
        },
    )
}

fn default_sync_policy_active() -> bool {
    true
}

pub fn parse_sync_policy_rules(
    rules: Vec<SyncPolicyRuleWire>,
) -> Result<Vec<crate::data_platform::SyncProfileRule>, String> {
    rules.into_iter().map(TryInto::try_into).collect()
}
