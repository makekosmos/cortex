use domain::sync_contracts::wire::{LanSyncMessage, SyncPolicyRuleWire};

fn policy_rule_wire(rule: &domain::sync_policy::SyncProfileRule) -> SyncPolicyRuleWire {
    SyncPolicyRuleWire {
        resource_kind: rule.resource_kind.clone(),
        resource_id: rule.resource_id.clone(),
        mode: rule.mode,
        filter: serde_json::from_str(rule.filter.canonical_json())
            .expect("validated policy filter is valid JSON"),
    }
}

fn policy_rule_from_wire(
    rule: SyncPolicyRuleWire,
) -> Result<domain::sync_policy::SyncProfileRule, String> {
    let filter_json = serde_json::to_string(&rule.filter).map_err(|error| error.to_string())?;
    Ok(domain::sync_policy::SyncProfileRule {
        resource_kind: rule.resource_kind,
        resource_id: rule.resource_id,
        mode: rule.mode,
        filter: domain::selective_sync::FilterV1::parse(&filter_json)?,
    })
}

pub fn sync_policy_message(profile: &domain::sync_policy::SyncProfile) -> LanSyncMessage {
    LanSyncMessage::SyncPolicy {
        revision: profile.revision,
        digest: profile.digest(),
        active: profile.active,
        rules: profile.rules.iter().map(policy_rule_wire).collect(),
    }
}

pub fn default_sync_policy_message() -> LanSyncMessage {
    let identity = domain::sync_policy::default_sync_policy_identity();
    LanSyncMessage::SyncPolicy {
        revision: identity.revision,
        digest: identity.digest,
        active: identity.active,
        rules: Vec::new(),
    }
}

pub fn sync_policy_identity(
    profile: Option<&domain::sync_policy::SyncProfile>,
) -> domain::sync_policy::PolicyIdentity {
    domain::sync_policy::sync_profile_identity(profile)
}

pub fn parse_sync_policy_rules(
    rules: Vec<SyncPolicyRuleWire>,
) -> Result<Vec<domain::sync_policy::SyncProfileRule>, String> {
    rules.into_iter().map(policy_rule_from_wire).collect()
}
