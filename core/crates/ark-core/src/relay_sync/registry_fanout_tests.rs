use super::*;
use crate::db::{init_schema, SqliteStorageBackend};
use crate::replication_storage::{PeerIdentityBackend, SyncRuntimeStorage};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

#[path = "registry_fanout/sqlite_catchup_tests.rs"]
mod sqlite_catchup_tests;
#[path = "registry_fanout/sqlite_policy_tests.rs"]
mod sqlite_policy_tests;
#[path = "registry_fanout/sqlite_test_support.rs"]
mod sqlite_test_support;

struct RegistryFanoutTransport {
    sent: Mutex<Vec<(String, LanSyncMessage)>>,
    operations: Mutex<Vec<String>>,
    fail_endpoint: Mutex<Option<String>>,
}

#[async_trait::async_trait]
impl SyncTransport for RegistryFanoutTransport {
    fn supports_registry_admission(&self) -> bool {
        true
    }

    async fn start(
        &self,
        _: tokio::sync::mpsc::UnboundedSender<TransportEvent>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn send(&self, _: LanSyncMessage) -> Result<(), String> {
        Ok(())
    }

    fn mark_peer_data_ready(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }

    fn unbind_peer(&self, device_id: &str) -> Result<(), String> {
        self.operations
            .lock()
            .unwrap()
            .push(format!("unbind:{device_id}"));
        Ok(())
    }

    async fn send_to_transport_peer(
        &self,
        endpoint: &str,
        message: LanSyncMessage,
    ) -> Result<crate::sync_transport::AddressedSendOutcome, String> {
        self.operations
            .lock()
            .unwrap()
            .push(format!("send:{endpoint}"));
        if self.fail_endpoint.lock().unwrap().as_deref() == Some(endpoint) {
            return Err("injected addressed-send failure".into());
        }
        self.sent.lock().unwrap().push((endpoint.into(), message));
        Ok(crate::sync_transport::AddressedSendOutcome::Sent)
    }

    fn disconnect_transport_peer(&self, endpoint: &str) -> Result<(), String> {
        self.operations
            .lock()
            .unwrap()
            .push(format!("disconnect:{endpoint}"));
        Ok(())
    }

    async fn stop(&self) {}
}

#[tokio::test]
async fn local_disable_revokes_before_sending_only_session_bound_registry_control() {
    use crate::domain::device_registry::{
        DeviceId, RegistryEvent, RegistryEventDraft, RegistryOperation,
    };

    let connection = Connection::open_in_memory().unwrap();
    init_schema(&connection).unwrap();
    let storage = Arc::new(SqliteStorageBackend::for_test(Arc::new(Mutex::new(
        connection,
    ))));
    let transport = Arc::new(RegistryFanoutTransport {
        sent: Mutex::new(Vec::new()),
        operations: Mutex::new(Vec::new()),
        fail_endpoint: Mutex::new(Some("active-endpoint".into())),
    });
    let relay = RelaySync::with_transport(
        storage as Arc<dyn SyncRuntimeStorage>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "registry-fanout-space".into(),
            device_id: "local-device".into(),
            device_name: "Local device".into(),
            auth_secret: None,
        },
        transport.clone(),
    );
    for (device_id, session_id, endpoint) in [
        ("disabled-device", "disabled-session", "disabled-endpoint"),
        ("active-device", "active-session", "active-endpoint"),
    ] {
        relay.peers.lock().await.insert(
            device_id.into(),
            RelayPeerState {
                device_name: device_id.into(),
                platform: None,
                app_version: None,
                authenticated: true,
                negotiated_protocol: None,
                registry_admission: RelayRegistryAdmission::Ready {
                    session_id: session_id.into(),
                    transport_public_key: endpoint.into(),
                },
            },
        );
    }
    let event = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("local-device").unwrap(),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: DeviceId::parse("disabled-device").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("local-device").to_string(),
            operation: RegistryOperation::Disable,
        },
        &ed25519_dalek::SigningKey::from_bytes(&[7; 32]),
    )
    .unwrap();

    let error = relay
        .publish_registry_event(event.clone())
        .await
        .unwrap_err();
    assert!(error.contains("failed to fan out registry event"));

    let sent = transport.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert!(sent.contains(&(
        "disabled-endpoint".into(),
        LanSyncMessage::RegistryEventUpdate {
            session_id: "disabled-session".into(),
            event,
        },
    )));
    {
        let operations = transport.operations.lock().unwrap();
        let unbind = operations
            .iter()
            .position(|entry| entry == "unbind:disabled-device")
            .unwrap();
        let first_send = operations
            .iter()
            .position(|entry| entry.starts_with("send:"))
            .unwrap();
        assert!(unbind < first_send);
        assert!(operations.contains(&"disconnect:disabled-endpoint".into()));
    }
    assert!(!relay.peers.lock().await.contains_key("disabled-device"));
    assert!(matches!(
        &relay.peers.lock().await["active-device"].registry_admission,
        RelayRegistryAdmission::Ready { session_id, .. } if session_id == "active-session"
    ));
}

#[tokio::test]
async fn inbound_registry_update_commits_once_fans_out_and_revokes_third_peer() {
    use crate::domain::device_registry::{
        DeviceId, DisplayName, RegistryEvent, RegistryEventDraft, RegistryOperation,
    };

    let connection = Connection::open_in_memory().unwrap();
    init_schema(&connection).unwrap();
    let storage = Arc::new(SqliteStorageBackend::for_test(Arc::new(Mutex::new(
        connection,
    ))));
    storage.set_device_id("local-device").unwrap();
    let author_iroh_key = iroh::SecretKey::generate();
    let disabled_iroh_key = iroh::SecretKey::generate();
    storage
        .enroll_paired_device(
            "local-device",
            "author-device",
            &author_iroh_key.public().to_string(),
        )
        .await
        .unwrap();
    storage
        .enroll_paired_device(
            "local-device",
            "disabled-device",
            &disabled_iroh_key.public().to_string(),
        )
        .await
        .unwrap();
    let author_signing_key = ed25519_dalek::SigningKey::from_bytes(&author_iroh_key.to_bytes());
    let author_id = DeviceId::parse("author-device").unwrap();
    let rename = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: author_id.clone(),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: author_id.clone(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("author-device").to_string(),
            operation: RegistryOperation::Rename {
                display_name: DisplayName::parse("Renamed author").unwrap(),
            },
        },
        &author_signing_key,
    )
    .unwrap();
    let previous_hash = rename
        .verify(&author_id, 1, &author_signing_key.verifying_key())
        .unwrap()
        .event_hash()
        .unwrap();
    let disable = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: author_id,
            actor_sequence: 2,
            previous_event_hash: Some(previous_hash),
            target_device_id: DeviceId::parse("disabled-device").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("author-device").to_string(),
            operation: RegistryOperation::Disable,
        },
        &author_signing_key,
    )
    .unwrap();
    let transport = Arc::new(RegistryFanoutTransport {
        sent: Mutex::new(Vec::new()),
        operations: Mutex::new(Vec::new()),
        fail_endpoint: Mutex::new(None),
    });
    let relay = RelaySync::with_transport(
        storage.clone() as Arc<dyn SyncRuntimeStorage>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "registry-fanout-space".into(),
            device_id: "local-device".into(),
            device_name: "Local device".into(),
            auth_secret: None,
        },
        transport.clone(),
    );
    for (device_id, session_id) in [
        ("author-device", "author-session"),
        ("disabled-device", "disabled-session"),
        ("observer-device", "observer-session"),
    ] {
        let endpoint = match device_id {
            "author-device" => author_iroh_key.public().to_string(),
            "disabled-device" => disabled_iroh_key.public().to_string(),
            _ => iroh::SecretKey::generate().public().to_string(),
        };
        relay.peers.lock().await.insert(
            device_id.into(),
            RelayPeerState {
                device_name: device_id.into(),
                platform: None,
                app_version: None,
                authenticated: true,
                negotiated_protocol: None,
                registry_admission: RelayRegistryAdmission::Ready {
                    session_id: session_id.into(),
                    transport_public_key: endpoint,
                },
            },
        );
    }
    relay
        .transport_keys
        .lock()
        .await
        .insert("author-device".into(), author_iroh_key.public().to_string());

    relay
        .handle_registry_event_update(
            "author-device",
            &author_iroh_key.public().to_string(),
            "author-session",
            rename.clone(),
        )
        .await
        .unwrap();
    let after_first_apply = transport.sent.lock().unwrap().len();
    assert_eq!(after_first_apply, 2);
    assert!(transport
        .sent
        .lock()
        .unwrap()
        .iter()
        .any(|(endpoint, message)| {
            endpoint == &disabled_iroh_key.public().to_string()
                && matches!(message, LanSyncMessage::RegistryEventUpdate { session_id, event }
                if session_id == "disabled-session" && *event == rename)
        }));

    relay
        .handle_registry_event_update(
            "author-device",
            &author_iroh_key.public().to_string(),
            "author-session",
            rename.clone(),
        )
        .await
        .unwrap();
    assert_eq!(transport.sent.lock().unwrap().len(), after_first_apply);
    assert!(relay
        .handle_registry_event_update(
            "author-device",
            &author_iroh_key.public().to_string(),
            "stale-session",
            disable.clone(),
        )
        .await
        .is_err());
    assert_eq!(transport.sent.lock().unwrap().len(), after_first_apply);

    relay
        .handle_registry_event_update(
            "author-device",
            &author_iroh_key.public().to_string(),
            "author-session",
            disable.clone(),
        )
        .await
        .unwrap();
    assert!(!relay.peers.lock().await.contains_key("disabled-device"));
    assert!(transport
        .operations
        .lock()
        .unwrap()
        .contains(&"unbind:disabled-device".to_owned()));
    assert!(transport
        .sent
        .lock()
        .unwrap()
        .iter()
        .any(|(endpoint, message)| {
            endpoint == &disabled_iroh_key.public().to_string()
                && matches!(message, LanSyncMessage::RegistryEventUpdate { session_id, event }
                if session_id == "disabled-session" && *event == disable)
        }));
    assert!(transport
        .sent
        .lock()
        .unwrap()
        .iter()
        .any(|(endpoint, message)| {
            matches!(message, LanSyncMessage::RegistryEventUpdate { session_id, event }
            if session_id == "observer-session" && *event == disable)
                && endpoint != &author_iroh_key.public().to_string()
                && endpoint != &disabled_iroh_key.public().to_string()
        }));
}

#[tokio::test]
async fn disable_during_exchange_is_applied_before_every_live_session_closes() {
    use crate::domain::device_registry::{
        DeviceId, RegistryEvent, RegistryEventDraft, RegistryOperation,
    };

    let connection = Connection::open_in_memory().unwrap();
    init_schema(&connection).unwrap();
    connection
        .execute(
            "INSERT INTO sync_kv (key, value)
             VALUES ('device_registry.local_device_id', 'local-device')",
            [],
        )
        .unwrap();
    let storage = Arc::new(SqliteStorageBackend::for_test(Arc::new(Mutex::new(
        connection,
    ))));
    storage.set_device_id("local-device").unwrap();
    let author_key = iroh::SecretKey::generate();
    storage
        .enroll_paired_device(
            "local-device",
            "author-device",
            &author_key.public().to_string(),
        )
        .await
        .unwrap();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&author_key.to_bytes());
    let event = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: DeviceId::parse("author-device").unwrap(),
            actor_sequence: 1,
            previous_event_hash: None,
            target_device_id: DeviceId::parse("local-device").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("author-device").to_string(),
            operation: RegistryOperation::Disable,
        },
        &signing_key,
    )
    .unwrap();
    let transport = Arc::new(RegistryFanoutTransport {
        sent: Mutex::new(Vec::new()),
        operations: Mutex::new(Vec::new()),
        fail_endpoint: Mutex::new(None),
    });
    let relay = RelaySync::with_transport(
        storage.clone() as Arc<dyn SyncRuntimeStorage>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "registry-fanout-space".into(),
            device_id: "local-device".into(),
            device_name: "Local device".into(),
            auth_secret: None,
        },
        transport.clone(),
    );
    let observer_endpoint = iroh::SecretKey::generate().public().to_string();
    relay.peers.lock().await.insert(
        "author-device".into(),
        RelayPeerState {
            device_name: "Author".into(),
            platform: None,
            app_version: None,
            authenticated: true,
            negotiated_protocol: None,
            registry_admission: RelayRegistryAdmission::Exchanging(RegistryExchangeState {
                session_id: "author-session".into(),
                transport_public_key: author_key.public().to_string(),
                retry_count: 0,
                incoming_page_count: None,
                next_incoming_page: 0,
                local_digest: None,
                remote_digest: None,
            }),
        },
    );
    relay.peers.lock().await.insert(
        "observer-device".into(),
        RelayPeerState {
            device_name: "Observer".into(),
            platform: None,
            app_version: None,
            authenticated: true,
            negotiated_protocol: None,
            registry_admission: RelayRegistryAdmission::Ready {
                session_id: "observer-session".into(),
                transport_public_key: observer_endpoint.clone(),
            },
        },
    );
    relay
        .transport_keys
        .lock()
        .await
        .insert("author-device".into(), author_key.public().to_string());

    let error = relay
        .handle_registry_event_update(
            "author-device",
            &author_key.public().to_string(),
            "author-session",
            event.clone(),
        )
        .await
        .unwrap_err();

    assert_eq!(error, "registry_sync_local_device_disabled");
    assert!(relay.peers.lock().await.is_empty());
    assert!(transport
        .sent
        .lock()
        .unwrap()
        .iter()
        .any(|(endpoint, message)| {
            endpoint == &observer_endpoint
                && matches!(message, LanSyncMessage::RegistryEventUpdate { session_id, event: sent }
                if session_id == "observer-session" && *sent == event)
        }));
    let operations = transport.operations.lock().unwrap().clone();
    assert!(operations.contains(&"unbind:author-device".into()));
    assert!(operations.contains(&"unbind:observer-device".into()));
    assert!(operations.contains(&format!("disconnect:{}", author_key.public())));
    assert!(operations.contains(&format!("disconnect:{observer_endpoint}")));
}
