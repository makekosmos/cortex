use super::*;
use crate::db::{DbHandle, DbOwner, SqliteStorageBackend};
use crate::replication_storage::{PeerIdentityBackend, SyncRuntimeStorage};
use crate::sync_bind::SyncBind;
use ark_domain::device_registry::{DeviceId, RegistryEvent, RegistryEventDraft, RegistryOperation};
use ark_net::iroh::RelayMode;
use std::sync::Arc;
use std::time::Duration;

struct RelayStorageFixture {
    storage: Arc<SqliteStorageBackend>,
    secret: ark_net::iroh::SecretKey,
    _database: ark_store::test_support::TestDatabase,
    owner: DbOwner,
}

async fn storage(device_id: &str) -> RelayStorageFixture {
    let database = ark_store::test_support::TestDatabase::create().unwrap();
    let (db, owner) = DbHandle::open(
        database.path().to_string_lossy().into_owned(),
        device_id.to_owned(),
    )
    .await
    .unwrap();
    let secret = crate::iroh_transport::load_or_generate_secret_key_via_actor(&db)
        .await
        .unwrap();
    let storage = Arc::new(SqliteStorageBackend::with_db_handle(db));
    storage.set_device_id(device_id).unwrap();
    RelayStorageFixture {
        storage,
        secret,
        _database: database,
        owner,
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
        space_id: "revoked-peer-catchup".into(),
        secret_key: Some(secret_key),
        peer_addr,
        peer_ticket: None,
        relay_mode: Some(RelayMode::Disabled),
        auth_secret: None,
        bind: SyncBind::Loopback,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn offline_revoked_iroh_peer_receives_tombstone_without_becoming_data_ready() {
    let mut peer_a = storage("device-A").await;
    let mut peer_b = storage("device-B").await;
    let storage_a = peer_a.storage.clone();
    let storage_b = peer_b.storage.clone();
    let key_a = peer_a.secret.clone();
    let key_b = peer_b.secret.clone();
    storage_a
        .enroll_paired_device("device-A", "device-B", &key_b.public().to_string())
        .await
        .unwrap();
    storage_b
        .enroll_paired_device("device-B", "device-A", &key_a.public().to_string())
        .await
        .unwrap();

    let (_, _, history) = storage_a
        .load_device_registry_events_page(0, None, 32)
        .await
        .unwrap();
    let enrollment = history
        .iter()
        .filter(|event| event.actor_device_id().as_str() == "device-A")
        .max_by_key(|event| event.actor_sequence())
        .unwrap();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&key_a.to_bytes());
    let previous_hash = enrollment
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
            actor_sequence: enrollment.actor_sequence() + 1,
            previous_event_hash: Some(previous_hash),
            target_device_id: DeviceId::parse("device-B").unwrap(),
            actor_key_epoch: 1,
            actor_admission_epoch: 1,
            hlc: crate::hlc::HLC::now("device-A").to_string(),
            operation: RegistryOperation::Disable,
        },
        &signing_key,
    )
    .unwrap();
    storage_a
        .apply_device_registry_events("device-A", std::slice::from_ref(&disable))
        .await
        .unwrap();

    let transport_a = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config(
        "device-A",
        key_a.clone(),
        None,
    )));
    let relay_a = RelaySync::with_iroh_transport(
        storage_a.clone() as Arc<dyn SyncRuntimeStorage>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "revoked-peer-catchup".into(),
            device_id: "device-A".into(),
            device_name: "device-A".into(),
            auth_secret: None,
        },
        transport_a.clone(),
    )
    .expect("Iroh RelaySync construction");
    relay_a.start().await.unwrap();

    let transport_b = Arc::new(crate::iroh_transport::IrohTransport::new(iroh_config(
        "device-B",
        key_b.clone(),
        transport_a.endpoint_addr(),
    )));
    let relay_b = RelaySync::with_iroh_transport(
        storage_b.clone() as Arc<dyn SyncRuntimeStorage>,
        RelaySyncConfig {
            relay_url: String::new(),
            relay_api_key: None,
            space_id: "revoked-peer-catchup".into(),
            device_id: "device-B".into(),
            device_name: "device-B".into(),
            auth_secret: None,
        },
        transport_b.clone(),
    )
    .expect("Iroh RelaySync construction");
    relay_b.start().await.unwrap();

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if storage_b
                .registry_device_is_enabled("device-B")
                .await
                .unwrap()
                == Some(false)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("offline device should receive and persist its signed disable tombstone");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!transport_b
        .registry()
        .is_data_ready_endpoint(&key_a.public()));
    assert!(!transport_a
        .registry()
        .is_data_ready_endpoint(&key_b.public()));

    relay_a.stop().await;
    relay_b.stop().await;
    peer_a
        .owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();
    peer_b
        .owner
        .shutdown(std::time::Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();
}
