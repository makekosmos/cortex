/// A receiver-owned policy cached by the sender for one authenticated peer.
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

impl AcceptedPeerSyncPolicy {
    pub fn identity(&self) -> crate::application::sync_storage::PolicyIdentity {
        crate::application::sync_storage::PolicyIdentity {
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

pub fn default_sync_policy_identity() -> crate::application::sync_storage::PolicyIdentity {
    crate::application::sync_storage::PolicyIdentity {
        revision: 0,
        digest: digest_sync_policy(&[]),
        active: false,
    }
}

/// Persist an authenticated receiver policy without allowing a stale session
/// or replay to roll back the per-peer outbound selection.
pub fn accept_peer_sync_policy(
    conn: &Connection,
    peer_device_id: &str,
    revision: i64,
    digest: &str,
    active: bool,
    rules: &[SyncProfileRule],
) -> Result<AcceptedPeerSyncPolicy, String> {
    if peer_device_id.is_empty() || peer_device_id.len() > 128 || revision < 1 {
        return Err("invalid accepted peer sync policy identity".into());
    }
    if rules.len() > 256 {
        return Err("too many accepted peer sync policy rules".into());
    }
    let mut identities = HashSet::new();
    for rule in rules {
        if !matches!(rule.resource_kind.as_str(), "type" | "dataset" | "blob")
            || rule.resource_id.is_empty()
            || rule.resource_id.len() > 128
            || !identities.insert((rule.resource_kind.as_str(), rule.resource_id.as_str()))
        {
            return Err("invalid accepted peer sync policy rule".into());
        }
    }
    let policy = AcceptedPeerSyncPolicy {
        peer_device_id: peer_device_id.to_owned(),
        revision,
        digest: digest.to_owned(),
        active,
        rules: rules.to_vec(),
    };
    if digest.len() != 64 || policy.computed_digest() != digest {
        return Err("accepted peer sync policy digest mismatch".into());
    }
    let current: Option<(i64, String, bool)> = conn
        .query_row(
            "SELECT revision, digest, active FROM accepted_peer_sync_policies \
             WHERE peer_device_id=?1",
            [peer_device_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some((current_revision, current_digest, current_active)) = current {
        if revision < current_revision {
            return Err("accepted peer sync policy revision is stale".into());
        }
        if revision == current_revision {
            if digest != current_digest || active != current_active {
                return Err("accepted peer sync policy revision conflicts".into());
            }
            return Ok(policy);
        }
    }
    let rules_json = canonical_policy_rules_json(rules);
    conn.execute(
        "INSERT INTO accepted_peer_sync_policies(peer_device_id,revision,digest,active,rules_json) \
         VALUES(?1,?2,?3,?4,?5) ON CONFLICT(peer_device_id) DO UPDATE SET \
         revision=excluded.revision,digest=excluded.digest,active=excluded.active,\
         rules_json=excluded.rules_json",
        rusqlite::params![peer_device_id, revision, digest, active, rules_json],
    )
    .map_err(|error| error.to_string())?;
    Ok(policy)
}

pub fn load_accepted_peer_sync_policy(
    conn: &Connection,
    peer_device_id: &str,
) -> Result<Option<AcceptedPeerSyncPolicy>, String> {
    let stored: Option<(i64, String, bool, String)> = conn
        .query_row(
            "SELECT revision,digest,active,rules_json FROM accepted_peer_sync_policies \
             WHERE peer_device_id=?1",
            [peer_device_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    stored
        .map(|(revision, digest, active, rules_json)| {
            let rules = parse_policy_rules_json(&rules_json)?;
            let policy = AcceptedPeerSyncPolicy {
                peer_device_id: peer_device_id.to_owned(),
                revision,
                digest,
                active,
                rules,
            };
            if policy.computed_digest() != policy.digest {
                return Err("persisted accepted peer sync policy digest mismatch".into());
            }
            Ok(policy)
        })
        .transpose()
}

fn canonical_policy_rules_json(rules: &[SyncProfileRule]) -> String {
    let mut rules = rules.iter().collect::<Vec<_>>();
    rules.sort_by(|left, right| {
        (&left.resource_kind, &left.resource_id).cmp(&(&right.resource_kind, &right.resource_id))
    });
    serde_json::to_string(
        &rules
            .into_iter()
            .map(|rule| {
                serde_json::json!({
                    "resourceKind": rule.resource_kind,
                    "resourceId": rule.resource_id,
                    "mode": sync_mode_name(rule.mode),
                    "filter": rule.filter.canonical_json(),
                })
            })
            .collect::<Vec<_>>(),
    )
    .expect("policy rules serialize")
}

fn parse_policy_rules_json(input: &str) -> Result<Vec<SyncProfileRule>, String> {
    let values: Vec<serde_json::Value> =
        serde_json::from_str(input).map_err(|error| error.to_string())?;
    values
        .into_iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| "invalid persisted sync policy rule".to_string())?;
            let resource_kind = object
                .get("resourceKind")
                .and_then(Value::as_str)
                .ok_or_else(|| "invalid persisted sync policy kind".to_string())?
                .to_owned();
            let resource_id = object
                .get("resourceId")
                .and_then(Value::as_str)
                .ok_or_else(|| "invalid persisted sync policy resource".to_string())?
                .to_owned();
            let mode = match object.get("mode").and_then(Value::as_str) {
                Some("full") => SyncMode::Full,
                Some("metadata") => SyncMode::Metadata,
                Some("none") => SyncMode::None,
                _ => return Err("invalid persisted sync policy mode".into()),
            };
            let filter_json = object
                .get("filter")
                .and_then(Value::as_str)
                .ok_or_else(|| "invalid persisted sync policy filter".to_string())?;
            Ok(SyncProfileRule {
                resource_kind,
                resource_id,
                mode,
                filter: FilterV1::parse(filter_json)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod accepted_peer_sync_policy_tests {
    use super::*;

    fn policy_rule(resource_id: &str) -> SyncProfileRule {
        SyncProfileRule {
            resource_kind: "type".into(),
            resource_id: resource_id.into(),
            mode: SyncMode::Full,
            filter: FilterV1::match_all(),
        }
    }

    #[test]
    fn accepted_peer_policy_is_monotonic_validated_and_durable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("peer-policy.db");
        let conn = Connection::open(&path).unwrap();
        ensure_schema(&conn).unwrap();
        let rules = vec![policy_rule("books")];
        let digest = digest_sync_policy(&rules);
        let accepted = accept_peer_sync_policy(&conn, "phone", 2, &digest, true, &rules).unwrap();
        assert_eq!(accepted.revision, 2);
        assert_eq!(
            accept_peer_sync_policy(&conn, "phone", 2, &digest, true, &rules).unwrap(),
            accepted
        );
        assert!(accept_peer_sync_policy(&conn, "phone", 2, &digest, false, &rules).is_err());
        assert!(accept_peer_sync_policy(&conn, "phone", 1, &digest, true, &rules).is_err());

        let conflicting_digest = digest_sync_policy(&[policy_rule("tasks")]);
        assert!(accept_peer_sync_policy(
            &conn,
            "phone",
            2,
            &conflicting_digest,
            true,
            &[policy_rule("tasks")]
        )
        .is_err());
        assert!(accept_peer_sync_policy(&conn, "phone", 3, "bad", true, &rules).is_err());

        let newer_digest = digest_sync_policy(&[policy_rule("tasks")]);
        accept_peer_sync_policy(
            &conn,
            "phone",
            3,
            &newer_digest,
            false,
            &[policy_rule("tasks")],
        )
        .unwrap();
        drop(conn);

        let reopened = Connection::open(path).unwrap();
        ensure_schema(&reopened).unwrap();
        let persisted = load_accepted_peer_sync_policy(&reopened, "phone")
            .unwrap()
            .unwrap();
        assert_eq!(persisted.revision, 3);
        assert_eq!(persisted.digest, newer_digest);
        assert!(!persisted.active);
        assert_eq!(persisted.rules[0].resource_id, "tasks");
        assert_eq!(
            load_accepted_peer_sync_policy(&reopened, "tablet").unwrap(),
            None
        );
    }

    #[test]
    fn accepted_policy_schema_upgrade_preserves_existing_policies_as_active() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE accepted_peer_sync_policies (
                peer_device_id TEXT PRIMARY KEY,
                revision INTEGER NOT NULL,
                digest TEXT NOT NULL,
                rules_json TEXT NOT NULL
            );",
        )
        .unwrap();
        ensure_schema(&conn).unwrap();
        let rules = vec![policy_rule("books")];
        let digest = digest_sync_policy(&rules);
        conn.execute(
            "INSERT INTO accepted_peer_sync_policies \
             (peer_device_id,revision,digest,rules_json) VALUES(?1,1,?2,?3)",
            rusqlite::params!["phone", digest, canonical_policy_rules_json(&rules)],
        )
        .unwrap();
        let loaded = load_accepted_peer_sync_policy(&conn, "phone")
            .unwrap()
            .unwrap();
        assert!(loaded.active);
        assert_eq!(loaded.rules, rules);
    }

    #[test]
    fn latest_sync_profile_includes_a_newer_inactive_profile() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        let rule = policy_rule("books");
        upsert_sync_profile(
            &conn,
            "phone",
            "select-books",
            "Books",
            true,
            None,
            std::slice::from_ref(&rule),
        )
        .unwrap();
        upsert_sync_profile(
            &conn,
            "phone",
            "select-books",
            "Books",
            false,
            Some(1),
            &[rule],
        )
        .unwrap();
        let latest = load_latest_sync_profile(&conn, "phone").unwrap().unwrap();
        assert_eq!(latest.revision, 2);
        assert!(!latest.active);
        assert_eq!(load_active_sync_profile(&conn, "phone").unwrap(), None);
    }

    #[test]
    fn sync_uses_active_profile_when_another_inactive_profile_changes() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        upsert_sync_profile(
            &conn,
            "phone",
            "selected",
            "Selected",
            true,
            None,
            &[policy_rule("books")],
        )
        .unwrap();
        upsert_sync_profile(
            &conn,
            "phone",
            "draft",
            "Draft",
            false,
            Some(1),
            &[policy_rule("games")],
        )
        .unwrap();
        let selected = load_sync_profile_for_sync(&conn, "phone").unwrap().unwrap();
        assert_eq!(selected.profile_id, "selected");
        assert!(selected.active);
        assert_eq!(selected.rules[0].resource_id, "books");
    }
}
