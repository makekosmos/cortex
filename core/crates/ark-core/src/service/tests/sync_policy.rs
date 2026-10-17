use super::*;
use crate::db::{CommandContext, LoadActiveLocalSyncProfileQuery, SetLocalSyncKeyValueCommand};

#[test]
fn sync_policy_request_parses_canonical_filter_inputs() {
    let request = serde_json::from_value::<Request>(json!({
        "operation": "sync_policy.set",
        "profileId": "phone",
        "name": "Phone",
        "active": true,
        "expectedRevision": 2,
        "rules": [{
            "resourceKind": "type",
            "resourceId": "com.kosmos.note",
            "mode": "full",
            "filter": {"version": 1, "where": {"exists": {"field": "title", "value": true}}}
        }]
    }))
    .expect("sync policy request should parse typed mode and filter input");

    let Request::SyncPolicySet {
        profile_id,
        expected_revision,
        rules,
        ..
    } = request
    else {
        panic!("expected sync_policy.set, got {request:?}");
    };
    assert_eq!(profile_id, "phone");
    assert_eq!(expected_revision, Some(2));
    assert_eq!(rules[0].mode, ark_domain::selective_sync::SyncMode::Full);
    assert!(rules[0].filter.is_object());
}

#[test]
fn sync_policy_digest_ignores_rule_order_and_tracks_rule_changes() {
    let rule = |resource_id: &str| ark_application::sync_policy::SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: resource_id.into(),
        mode: ark_domain::selective_sync::SyncMode::Full,
        filter: ark_domain::selective_sync::FilterV1::match_all(),
    };
    let profile = ark_application::sync_policy::SyncProfile {
        profile_id: "phone".into(),
        device_id: "phone-device".into(),
        revision: 1,
        name: "Phone".into(),
        active: true,
        rules: vec![rule("com.kosmos.note"), rule("com.kosmos.book")],
    };
    let mut reordered = profile.clone();
    reordered.rules.reverse();
    assert_eq!(profile.digest(), reordered.digest());

    let mut changed = profile;
    changed.rules[0].mode = ark_domain::selective_sync::SyncMode::Metadata;
    assert_ne!(changed.digest(), reordered.digest());
}

#[tokio::test]
async fn engine_sync_policy_api_round_trips_active_device_policy() {
    let fixture = service_fixture();
    handle_request(
        &fixture.state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("initialize service database");
    get_db_handle(&fixture.state)
        .unwrap()
        .submit(
            CommandContext::engine(),
            SetLocalSyncKeyValueCommand::new(
                crate::db::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY.into(),
                "phone-device".into(),
            ),
        )
        .await
        .unwrap();

    let saved = handle_request(
        &fixture.state,
        Request::SyncPolicySet {
            profile_id: "phone-profile".into(),
            name: "Phone".into(),
            active: true,
            expected_revision: None,
            rules: vec![SyncPolicyRuleInput {
                resource_kind: "type".into(),
                resource_id: "com.kosmos.note".into(),
                mode: ark_domain::selective_sync::SyncMode::Metadata,
                filter: json!({
                    "version": 1,
                    "where": {"exists": {"field": "title", "value": true}}
                }),
            }],
        },
    )
    .await
    .expect("save the device-owned selection through Engine");
    assert_eq!(saved["revision"], 1);
    assert_eq!(saved["deviceId"], "phone-device");
    assert_eq!(saved["rules"][0]["mode"], "metadata");
    assert_eq!(saved["rules"][0]["filter"]["version"], 1);
    assert_eq!(saved["digest"].as_str().unwrap().len(), 64);

    let loaded = handle_request(&fixture.state, Request::SyncPolicyGet)
        .await
        .expect("load the active device-owned selection");
    assert_eq!(loaded, saved);

    let second = handle_request(
        &fixture.state,
        Request::SyncPolicySet {
            profile_id: "phone-minimal".into(),
            name: "Minimal".into(),
            active: true,
            expected_revision: Some(1),
            rules: vec![],
        },
    )
    .await
    .expect("activating a profile replaces this device's prior active selection");
    assert_eq!(second["profileId"], "phone-minimal");
    assert_eq!(second["revision"], 2);
    assert_ne!(second["digest"], saved["digest"]);
    let loaded = handle_request(&fixture.state, Request::SyncPolicyGet)
        .await
        .expect("load the newly active selection");
    assert_eq!(loaded["profileId"], "phone-minimal");

    let durable = get_db_handle(&fixture.state)
        .unwrap()
        .query(
            CommandContext::engine(),
            LoadActiveLocalSyncProfileQuery::new("phone-device".into()),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(durable.active);
    assert_eq!(durable.profile_id, "phone-minimal");
}

#[tokio::test]
async fn sync_selection_does_not_revoke_an_application_grant() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    handle_request(
        &state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .expect("initialize service database");
    get_db_handle(&state)
        .unwrap()
        .submit(
            CommandContext::engine(),
            SetLocalSyncKeyValueCommand::new(
                crate::db::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY.into(),
                "phone-device".into(),
            ),
        )
        .await
        .unwrap();

    let authorization = serde_json::from_value(json!({
        "operation": "integration.persist_node_authorization",
        "authorization_operation": "authorize",
        "node": {
            "node_id": "app-node",
            "key_fingerprint": "app-node-fingerprint",
            "signing_public_key": "app-node-signing-key",
            "encryption_public_key": "app-node-encryption-key",
            "transport_public_key": null,
            "grant_epoch": 1,
            "status": "active",
            "authorized_at": "2026-09-06T00:00:00Z",
            "revoked_at": null,
            "revocation_epoch": null,
            "revision": 1,
            "hlc": "2026-09-06T00:00:00.000Z:000001:app-node"
        },
        "grant": {
            "integration_id": "notes-app",
            "node_id": "app-node",
            "node_encryption_key": "app-node-encryption-key",
            "grant_epoch": 1,
            "status": "active",
            "authorized_at": "2026-09-06T00:00:00Z",
            "revoked_at": null,
            "revision": 1,
            "hlc": "2026-09-06T00:00:00.000Z:000002:app-node"
        },
        "device_id": "phone-device"
    }))
    .expect("parse application authorization request");
    handle_request(&state, authorization)
        .await
        .expect("grant access to the application");

    handle_request(
        &state,
        Request::SyncPolicySet {
            profile_id: "phone-minimal".into(),
            name: "No shared notes".into(),
            active: true,
            expected_revision: None,
            rules: vec![crate::service::SyncPolicyRuleInput {
                resource_kind: "type".into(),
                resource_id: "com.kosmos.note".into(),
                mode: ark_domain::selective_sync::SyncMode::None,
                filter: json!({"version": 1, "where": {"and": []}}),
            }],
        },
    )
    .await
    .expect("exclude notes from device sync");

    let grant = with_conn(&state, |connection| {
        crate::db::load_integration_node_grant(connection, "notes-app", "app-node")
    })
    .expect("load application grant after changing sync selection")
    .expect("sync selection must preserve the application grant");
    assert_eq!(
        grant.status,
        crate::integration_replication::GrantStatus::Active
    );
}
