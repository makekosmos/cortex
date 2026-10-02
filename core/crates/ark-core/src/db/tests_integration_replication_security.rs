use crate::integration_replication::{
    IntegrationReplicationChange as Change, IntegrationReplicationEntity as Entity,
};
use chrono::{Duration, Utc};

#[test]
fn legacy_opaque_sync_cursor_is_scrubbed_before_configuration_load() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE integration_configurations (
           integration_id TEXT PRIMARY KEY, provider TEXT NOT NULL, account_subject TEXT NOT \
           NULL,
           public_scopes_json TEXT NOT NULL, public_settings_json TEXT NOT NULL, enabled \
           INTEGER NOT NULL,
           sync_cursor TEXT, revision INTEGER NOT NULL, hlc TEXT NOT NULL
         );
         INSERT INTO integration_configurations VALUES
           ('opaque', 'fatsecret', 'a', '[]', '{}', 1, 'bearer-token', 1, \
           '2026-08-30T00:00:00.000Z:000001:node-a'),
           ('numeric', 'fatsecret', 'b', '[]', '{}', 1, '42', 1, \
           '2026-08-30T00:00:00.000Z:000001:node-a');",
    )
    .unwrap();
    assert_eq!(
        load_integration_configuration(&conn, "opaque")
            .unwrap()
            .unwrap()
            .sync_cursor,
        None
    );
    let persisted: Option<String> = conn
        .query_row(
            concat!(
                "SELECT sync_cursor FROM integration_configurations WHERE integration_id = ",
                "'opaque'"
            ),
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(persisted, None);
    assert_eq!(
        load_integration_configuration(&conn, "numeric")
            .unwrap()
            .unwrap()
            .sync_cursor,
        Some(42)
    );
}

fn trusted_target(seed: u8) -> (rusqlite::Connection, SigningKey) {
    let target = integration_test_db();
    let origin_key = SigningKey::from_bytes(&[seed; 32]);
    let recipient_key = SigningKey::from_bytes(&[seed + 1; 32]);
    let origin = replication_node("node-a", &origin_key, "enc-a", "node-a");
    let recipient = replication_node("node-b", &recipient_key, "enc-b", "node-a");
    upsert_authorized_node(&target, &origin, "bootstrap").unwrap();
    upsert_authorized_node(&target, &recipient, "bootstrap").unwrap();
    upsert_integration_node_grant(
        &target,
        &replication_grant("node-a", "enc-a", 2),
        "bootstrap",
    )
    .unwrap();
    upsert_integration_node_grant(
        &target,
        &replication_grant("node-b", "enc-b", 2),
        "bootstrap",
    )
    .unwrap();
    (target, origin_key)
}

fn grant_for(integration_id: &str, node_id: &str, encryption_key: &str) -> IntegrationNodeGrant {
    let mut grant = replication_grant(node_id, encryption_key, 2);
    grant.integration_id = integration_id.into();
    grant.hlc = "2026-08-31T00:00:00.000Z:000002:node-a".into();
    grant
}

#[test]
fn signed_apply_rejects_payload_authorization_without_preexisting_local_trust() {
    let target = integration_test_db();
    let origin_key = SigningKey::from_bytes(&[51; 32]);
    let recipient_key = SigningKey::from_bytes(&[52; 32]);
    upsert_authorized_node(
        &target,
        &replication_node("node-a", &origin_key, "enc-a", "node-a"),
        "bootstrap",
    )
    .unwrap();
    upsert_authorized_node(
        &target,
        &replication_node("node-b", &recipient_key, "enc-b", "node-a"),
        "bootstrap",
    )
    .unwrap();
    let frame = sign_replication_batch(
        &origin_key,
        2,
        "message-bootstrap",
        vec![
            Change {
                entity: Entity::IntegrationConfiguration(integration_config(serde_json::json!({}))),
                vector_hlc: "2026-08-31T00:00:00.000Z:000003:node-a".into(),
            },
            Change {
                entity: Entity::IntegrationNodeGrant(replication_grant("node-a", "enc-a", 2)),
                vector_hlc: "2026-08-31T00:00:00.000Z:000004:node-a".into(),
            },
            Change {
                entity: Entity::IntegrationNodeGrant(replication_grant("node-b", "enc-b", 2)),
                vector_hlc: "2026-08-31T00:00:00.000Z:000005:node-a".into(),
            },
        ],
    );
    assert!(matches!(
        apply_signed_integration_changes(&target, &frame, "space-a", "node-b", None, 15),
        Err(SignedSyncError::Apply(error)) if error.contains("pre-existing integration grant")
    ));
    assert!(load_integration_configuration(&target, "integration-a")
        .unwrap()
        .is_none());
}

#[test]
fn signed_apply_rejects_new_node_authorization_without_local_trust() {
    let (target, origin_key) = trusted_target(61);
    let new_key = SigningKey::from_bytes(&[63; 32]);
    let frame = sign_replication_batch(
        &origin_key,
        2,
        "message-new-node",
        vec![
            Change {
                entity: Entity::IntegrationConfiguration(integration_config(serde_json::json!({}))),
                vector_hlc: "2026-08-31T00:00:00.000Z:000003:node-a".into(),
            },
            Change {
                entity: Entity::AuthorizedNode(replication_node(
                    "node-c", &new_key, "enc-c", "node-a",
                )),
                vector_hlc: "2026-08-31T00:00:00.000Z:000004:node-a".into(),
            },
            Change {
                entity: Entity::IntegrationNodeGrant(replication_grant("node-c", "enc-c", 2)),
                vector_hlc: "2026-08-31T00:00:00.000Z:000005:node-a".into(),
            },
        ],
    );
    assert!(matches!(
        apply_signed_integration_changes(&target, &frame, "space-a", "node-b", None, 15),
        Err(SignedSyncError::Apply(error)) if error.contains("pre-existing authorization")
    ));
    assert!(load_authorized_node(&target, "node-c").unwrap().is_none());
}

#[test]
fn signed_apply_cannot_use_trust_from_another_integration() {
    let (target, origin_key) = trusted_target(66);
    let third_key = SigningKey::from_bytes(&[68; 32]);
    let third = replication_node("node-c", &third_key, "enc-c", "node-a");
    upsert_authorized_node(&target, &third, "bootstrap").unwrap();
    upsert_integration_node_grant(
        &target,
        &grant_for("integration-a", "node-c", "enc-c"),
        "bootstrap",
    )
    .unwrap();
    upsert_integration_node_grant(
        &target,
        &grant_for("integration-b", "node-a", "enc-a"),
        "bootstrap",
    )
    .unwrap();
    upsert_integration_node_grant(
        &target,
        &grant_for("integration-b", "node-b", "enc-b"),
        "bootstrap",
    )
    .unwrap();
    let frame = sign_replication_batch(
        &origin_key,
        2,
        "message-cross-integration",
        vec![
            Change {
                entity: Entity::IntegrationConfiguration(integration_config(serde_json::json!({}))),
                vector_hlc: "2026-08-31T00:00:00.000Z:000003:node-a".into(),
            },
            Change {
                entity: Entity::IntegrationNodeGrant(grant_for("integration-b", "node-c", "enc-c")),
                vector_hlc: "2026-08-31T00:00:00.000Z:000004:node-a".into(),
            },
        ],
    );
    assert!(matches!(
        apply_signed_integration_changes(&target, &frame, "space-a", "node-b", None, 15),
        Err(SignedSyncError::Apply(error)) if error.contains("pre-existing integration grant")
    ));
    assert!(
        load_integration_node_grant(&target, "integration-b", "node-c")
            .unwrap()
            .is_none()
    );
}

#[test]
fn signed_apply_rejects_hlcs_not_owned_by_the_signer_or_too_far_in_the_future() {
    let (target, origin_key) = trusted_target(71);
    let mut foreign = integration_config(serde_json::json!({}));
    foreign.hlc = "2026-08-31T00:00:00.000Z:000001:node-c".into();
    let frame = sign_replication_batch(
        &origin_key,
        2,
        "message-foreign-hlc",
        vec![Change {
            entity: Entity::IntegrationConfiguration(foreign),
            vector_hlc: "2026-08-31T00:00:00.000Z:000002:node-a".into(),
        }],
    );
    assert!(matches!(
        apply_signed_integration_changes(&target, &frame, "space-a", "node-b", None, 15),
        Err(SignedSyncError::Apply(error)) if error.contains("does not match signed origin")
    ));
    let future = (Utc::now() + Duration::hours(1))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let mut remote = integration_config(serde_json::json!({}));
    remote.hlc = format!("{future}:000001:node-a");
    let frame = sign_replication_batch(
        &origin_key,
        2,
        "message-future-hlc",
        vec![Change {
            entity: Entity::IntegrationConfiguration(remote),
            vector_hlc: format!("{future}:000002:node-a"),
        }],
    );
    assert!(matches!(
        apply_signed_integration_changes(&target, &frame, "space-a", "node-b", None, 16),
        Err(SignedSyncError::Apply(error)) if error.contains("too far in the future")
    ));
}
