use super::super::*;
use crate::db::{init_schema, SqliteStorageBackend};
use crate::replication_storage::{PeerIdentityBackend, SyncRuntimeStorage};
use crate::sync_bind::SyncBind;
use ark_domain::device_registry::{
    DeviceId, DisplayName, RegistryEvent, RegistryEventDraft, RegistryOperation,
};
use ark_net::iroh::RelayMode;
use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

async fn storage_for(device_id: &str) -> (Arc<SqliteStorageBackend>, ark_net::iroh::SecretKey) {
    storage_for_with_policy(device_id, Vec::new()).await
}

async fn storage_for_with_policy(
    device_id: &str,
    rules: Vec<ark_application::sync_policy::SyncProfileRule>,
) -> (Arc<SqliteStorageBackend>, ark_net::iroh::SecretKey) {
    let connection = Connection::open_in_memory().unwrap();
    init_schema(&connection).unwrap();
    connection
        .execute(
            "INSERT INTO sync_kv (key, value)
             VALUES ('device_registry.local_device_id', ?1)",
            [device_id],
        )
        .unwrap();
    if !rules.is_empty() {
        ark_store::data_platform::upsert_sync_profile(
            &connection,
            device_id,
            "phone",
            "Phone",
            true,
            None,
            &rules,
        )
        .unwrap();
    }
    let secret = crate::iroh_transport::load_or_generate_secret_key(&connection).unwrap();
    let storage = Arc::new(SqliteStorageBackend::for_test(Arc::new(Mutex::new(
        connection,
    ))));
    (storage, secret)
}

fn relay_config(device_id: &str) -> RelaySyncConfig {
    RelaySyncConfig {
        relay_url: String::new(),
        relay_api_key: None,
        space_id: "registry-live-iroh-test".into(),
        device_id: device_id.into(),
        device_name: device_id.into(),
        auth_secret: None,
    }
}

fn iroh_config(
    device_id: &str,
    secret_key: ark_net::iroh::SecretKey,
    peer_addr: Option<ark_net::iroh::EndpointAddr>,
) -> crate::iroh_transport::IrohConfig {
    crate::iroh_transport::IrohConfig {
        device_id: device_id.into(),
        device_name: device_id.into(),
        space_id: "registry-live-iroh-test".into(),
        secret_key: Some(secret_key),
        peer_addr,
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
        bind: SyncBind::Loopback,
    }
}

async fn wait_for_registry_ready(transports: &[(&crate::iroh_transport::IrohTransport, String)]) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if transports.iter().all(|(transport, peer)| {
            peer.parse()
                .is_ok_and(|endpoint| transport.registry().is_data_ready_endpoint(&endpoint))
        }) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Iroh registry exchange did not make every peer data-ready"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn live_registry_events_reach_iroh_peers_and_disable_revokes_only_target() {
    let (storage_a, key_a) = storage_for("device-A").await;
    let (storage_b, key_b) = storage_for("device-B").await;
    let (storage_c, key_c) = storage_for("device-C").await;
    let endpoint_a = key_a.public();
    let endpoint_b = key_b.public();
    let endpoint_c = key_c.public();

    for (storage, local_id, peer_id, peer_key) in [
        (&storage_a, "device-A", "device-B", endpoint_b),
        (&storage_a, "device-A", "device-C", endpoint_c),
        (&storage_b, "device-B", "device-A", endpoint_a),
        (&storage_b, "device-B", "device-C", endpoint_c),
        (&storage_c, "device-C", "device-A", endpoint_a),
        (&storage_c, "device-C", "device-B", endpoint_b),
    ] {
        storage
            .enroll_paired_device(local_id, peer_id, &peer_key.to_string())
            .await
            .unwrap();
    }

    let transport_a = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config(
        "device-A",
        key_a.clone(),
        None,
    )));
    let relay_a = RelaySync::with_transport(
        storage_a.clone() as Arc<dyn SyncRuntimeStorage>,
        relay_config("device-A"),
        transport_a.clone(),
    );
    relay_a.start().await.unwrap();

    let transport_b = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config(
        "device-B",
        key_b,
        transport_a.endpoint_addr(),
    )));
    let relay_b = RelaySync::with_transport(
        storage_b.clone() as Arc<dyn SyncRuntimeStorage>,
        relay_config("device-B"),
        transport_b.clone(),
    );
    relay_b.start().await.unwrap();

    let transport_c = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config(
        "device-C",
        key_c,
        transport_a.endpoint_addr(),
    )));
    let relay_c = RelaySync::with_transport(
        storage_c.clone() as Arc<dyn SyncRuntimeStorage>,
        relay_config("device-C"),
        transport_c.clone(),
    );
    let received_b = Arc::new(AtomicBool::new(false));
    let received_b_callback = received_b.clone();
    relay_b
        .set_on_change(Arc::new(move |_| {
            received_b_callback.store(true, Ordering::Relaxed);
        }))
        .await;
    let registry_events_b = Arc::new(Mutex::new(Vec::new()));
    let registry_events_b_callback = registry_events_b.clone();
    relay_b
        .set_on_registry_events(Arc::new(move |events| {
            registry_events_b_callback
                .lock()
                .unwrap()
                .extend(events.iter().map(|event| event.event_id().to_owned()));
        }))
        .await;
    let received_c = Arc::new(AtomicBool::new(false));
    let received_c_callback = received_c.clone();
    relay_c
        .set_on_change(Arc::new(move |_| {
            received_c_callback.store(true, Ordering::Relaxed);
        }))
        .await;
    relay_c.start().await.unwrap();

    wait_for_registry_ready(&[
        (&transport_a, endpoint_b.to_string()),
        (&transport_a, endpoint_c.to_string()),
        (&transport_b, endpoint_a.to_string()),
        (&transport_c, endpoint_a.to_string()),
    ])
    .await;

    let (_, _, history) = storage_a
        .load_device_registry_events_page(0, None, 32)
        .await
        .unwrap();
    let latest = history
        .iter()
        .filter(|event| event.actor_device_id().as_str() == "device-A")
        .max_by_key(|event| event.actor_sequence())
        .unwrap();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&key_a.to_bytes());
    let previous_event_hash = latest
        .verify(
            &DeviceId::parse("device-A").unwrap(),
            1,
            &signing_key.verifying_key(),
        )
        .unwrap()
        .event_hash()
        .unwrap();
    let event = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("device-A").unwrap(),
            actor_sequence: latest.actor_sequence() + 1,
            previous_event_hash: Some(previous_event_hash),
            target_device_id: DeviceId::parse("device-C").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("device-A").to_string(),
            operation: RegistryOperation::Rename {
                display_name: DisplayName::parse("Cortex tablet").unwrap(),
            },
        },
        &signing_key,
    )
    .unwrap();
    let applied = storage_a
        .apply_device_registry_events("device-A", std::slice::from_ref(&event))
        .await
        .unwrap();
    assert_eq!(applied.applied_events, vec![event.clone()]);
    relay_a.publish_registry_event(event.clone()).await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let (_, _, events_b) = storage_b
            .load_device_registry_events_page(0, None, 32)
            .await
            .unwrap();
        let (_, _, events_c) = storage_c
            .load_device_registry_events_page(0, None, 32)
            .await
            .unwrap();
        let seen_b = events_b.iter().any(|candidate| candidate == &event);
        let seen_c = events_c.iter().any(|candidate| candidate == &event);
        if seen_b && seen_c {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "signed rename did not reach both Iroh peers"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        registry_events_b
            .lock()
            .unwrap()
            .iter()
            .filter(|event_id| event_id.as_str() == event.event_id())
            .count(),
        1,
        "newly applied Iroh registry event must propagate to service-owned transports exactly once"
    );

    let (_, _, updated_history) = storage_a
        .load_device_registry_events_page(0, None, 32)
        .await
        .unwrap();
    let latest = updated_history
        .iter()
        .filter(|candidate| candidate.actor_device_id().as_str() == "device-A")
        .max_by_key(|candidate| candidate.actor_sequence())
        .unwrap();
    let previous_event_hash = latest
        .verify(
            &DeviceId::parse("device-A").unwrap(),
            1,
            &signing_key.verifying_key(),
        )
        .unwrap()
        .event_hash()
        .unwrap();
    let disable = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("device-A").unwrap(),
            actor_sequence: latest.actor_sequence() + 1,
            previous_event_hash: Some(previous_event_hash),
            target_device_id: DeviceId::parse("device-C").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("device-A").to_string(),
            operation: RegistryOperation::Disable,
        },
        &signing_key,
    )
    .unwrap();
    let applied = storage_a
        .apply_device_registry_events("device-A", std::slice::from_ref(&disable))
        .await
        .unwrap();
    assert_eq!(applied.applied_events, vec![disable.clone()]);
    relay_a
        .publish_registry_event(disable.clone())
        .await
        .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let (_, _, events_b) = storage_b
            .load_device_registry_events_page(0, None, 32)
            .await
            .unwrap();
        let (_, _, events_c) = storage_c
            .load_device_registry_events_page(0, None, 32)
            .await
            .unwrap();
        if events_b.iter().any(|candidate| candidate == &disable)
            && events_c.iter().any(|candidate| candidate == &disable)
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "signed disable did not reach both Iroh peers"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!transport_a.registry().is_data_ready_endpoint(&endpoint_c));

    received_b.store(false, Ordering::Relaxed);
    received_c.store(false, Ordering::Relaxed);
    relay_a
        .broadcast_live_change(crate::types::SyncEntity {
            id: "after-disable".into(),
            entity_type: "todo".into(),
            data: serde_json::Map::new(),
            hlc: crate::hlc::HLC::now("device-A").to_string(),
            deleted: None,
            origin_device_id: Some("device-A".into()),
            origin_seq: None,
        })
        .await
        .unwrap();
    let delivery_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !received_b.load(Ordering::Relaxed) {
        assert!(
            tokio::time::Instant::now() < delivery_deadline,
            "active peer stopped receiving live data after another peer was disabled"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        !received_c.load(Ordering::Relaxed),
        "disabled peer must not receive user data"
    );

    relay_a.stop().await;
    relay_b.stop().await;
    relay_c.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn negotiated_sync_policy_is_sent_and_persisted_for_authenticated_iroh_peer() {
    const AUTH_SECRET: &str = "selective-sync-policy-test-secret";
    let (storage_a, key_a) = storage_for_with_policy(
        "policy-A",
        vec![ark_application::sync_policy::SyncProfileRule {
            resource_kind: "type".into(),
            resource_id: "com.example.note".into(),
            mode: ark_domain::selective_sync::SyncMode::Metadata,
            filter: ark_domain::selective_sync::FilterV1::match_all(),
        }],
    )
    .await;
    let (storage_b, key_b) = storage_for("policy-B").await;
    let endpoint_a = key_a.public().to_string();
    let endpoint_b = key_b.public().to_string();
    storage_a
        .enroll_paired_device("policy-A", "policy-B", &key_b.public().to_string())
        .await
        .unwrap();
    storage_b
        .enroll_paired_device("policy-B", "policy-A", &key_a.public().to_string())
        .await
        .unwrap();

    let mut relay_config_a = relay_config("policy-A");
    relay_config_a.auth_secret = Some(AUTH_SECRET.into());
    let mut relay_config_b = relay_config("policy-B");
    relay_config_b.auth_secret = Some(AUTH_SECRET.into());
    let mut iroh_config_a = iroh_config("policy-A", key_a.clone(), None);
    iroh_config_a.auth_secret = Some(AUTH_SECRET.into());
    let transport_a = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config_a));
    let relay_a = RelaySync::with_transport(
        storage_a as Arc<dyn SyncRuntimeStorage>,
        relay_config_a,
        transport_a.clone(),
    );
    relay_a.start().await.unwrap();

    let mut iroh_config_b = iroh_config("policy-B", key_b, transport_a.endpoint_addr());
    iroh_config_b.auth_secret = Some(AUTH_SECRET.into());
    let transport_b = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config_b));
    let relay_b = RelaySync::with_transport(
        storage_b.clone() as Arc<dyn SyncRuntimeStorage>,
        relay_config_b,
        transport_b.clone(),
    );
    relay_b.start().await.unwrap();

    wait_for_registry_ready(&[(&transport_a, endpoint_b), (&transport_b, endpoint_a)]).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(accepted) = storage_b
            .load_accepted_peer_sync_policy("policy-A")
            .await
            .unwrap()
        {
            assert_eq!(accepted.revision, 1);
            assert_eq!(accepted.rules.len(), 1);
            assert_eq!(accepted.rules[0].resource_id, "com.example.note");
            assert_eq!(
                accepted.digest,
                ark_application::sync_policy::SyncProfile {
                    profile_id: "phone".into(),
                    device_id: "policy-A".into(),
                    revision: 1,
                    name: "Phone".into(),
                    active: true,
                    rules: accepted.rules.clone(),
                }
                .digest()
            );
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "negotiated sync policy was not persisted by the authenticated peer"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    relay_a.stop().await;
    relay_b.stop().await;
}
