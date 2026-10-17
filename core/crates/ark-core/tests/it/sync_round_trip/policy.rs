use super::{pair_storages, pick_port, SyncClient, SyncServer};
use ark_core::application::sync_storage::{
    SyncEntityMutationPort, SyncEntityReadPort, SyncReadClass,
};
use ark_core::data_platform::{self, FilterV1, SyncMode, SyncProfileRule};
use ark_core::replication_storage::PeerIdentityBackend;
use ark_core::sync_server::SyncRuntimeStorage;
use ark_core::types::{ArkObject, ObjectType, PeerRecord, SyncEntity};
use ark_core::{db, SqliteStorageBackend};
use rusqlite::Connection;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn policy_rule(mode: SyncMode) -> SyncProfileRule {
    SyncProfileRule {
        resource_kind: "type".into(),
        resource_id: "book".into(),
        mode,
        filter: FilterV1::match_all(),
    }
}

fn make_policy_storage(
    device_id: &str,
    profile_rules: Option<Vec<SyncProfileRule>>,
) -> (Arc<SqliteStorageBackend>, String, Arc<Mutex<Connection>>) {
    let conn = Connection::open_in_memory().expect("in-memory SQLite");
    db::init_schema(&conn).expect("initialize test schema");
    db::upsert_object_type(
        &conn,
        &ObjectType {
            id: "book".into(),
            name: "Book".into(),
            schema_json: "{}".into(),
            ui_schema_json: "{}".into(),
            created_at: "2026-10-16T00:00:00.000Z".into(),
            updated_at: "2026-10-16T00:00:00.000Z".into(),
            system_locked: false,
        },
    )
    .expect("register book type");
    if let Some(rules) = profile_rules {
        data_platform::upsert_sync_profile(
            &conn,
            device_id,
            &format!("profile-{device_id}"),
            "Device sync selection",
            true,
            None,
            &rules,
        )
        .expect("persist device sync policy");
    }
    let endpoint = ark_core::iroh_transport::load_or_generate_secret_key(&conn)
        .expect("device signing key")
        .public()
        .to_string();
    let conn = Arc::new(Mutex::new(conn));
    let storage = Arc::new(SqliteStorageBackend::for_test(conn.clone()));
    storage.set_device_id(device_id).unwrap();
    (storage, endpoint, conn)
}

fn book_entity(title: &str, counter: u64) -> SyncEntity {
    let object = ArkObject {
        id: "book-1".into(),
        type_id: "book".into(),
        type_version: "0.0.0-legacy".into(),
        title: title.into(),
        content_json: json!({"text":title}),
        props_json: json!({}),
        created_at: "2026-10-16T00:00:00.000Z".into(),
        updated_at: format!("2026-10-16T00:00:{counter:02}.000Z"),
        deleted_at: None,
    };
    let mut data = serde_json::to_value(object)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    data.remove("id");
    SyncEntity {
        entity_type: "object".into(),
        id: "book-1".into(),
        data,
        hlc: format!("2026-10-16T00:00:{counter:02}.000Z:{counter:06}:device-a"),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

fn stored_book_title(conn: &Connection) -> String {
    db::get_object(conn, "book-1")
        .unwrap()
        .expect("book row should exist")
        .title
}

fn client_for(
    storage: &Arc<SqliteStorageBackend>,
    port: u16,
    auth_secret: &str,
) -> Arc<SyncClient> {
    Arc::new(SyncClient::new(
        storage.clone() as Arc<dyn SyncRuntimeStorage>,
        PeerRecord {
            device_id: "device-a".into(),
            device_name: "Device A".into(),
            addresses: vec![format!("127.0.0.1:{port}")],
            last_seen: chrono::Utc::now().to_rfc3339(),
            last_address: None,
            platform: None,
            app_version: None,
        },
        "device-b".into(),
        "Device B".into(),
        "policy-transfer-test".into(),
        Vec::new(),
        Some(auth_secret.into()),
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn receiver_policy_stops_transfer_and_widening_resumes_after_reconnect() {
    let (source, source_endpoint, source_conn) = make_policy_storage("device-a", None);
    let (receiver, receiver_endpoint, receiver_conn) =
        make_policy_storage("device-b", Some(vec![policy_rule(SyncMode::None)]));
    source
        .apply_entity(&book_entity("Original source version", 1))
        .await
        .unwrap();
    receiver
        .apply_entity(&book_entity("Local copy before narrowing", 0))
        .await
        .unwrap();
    pair_storages(
        &source,
        "device-a",
        &source_endpoint,
        &receiver,
        "device-b",
        &receiver_endpoint,
    )
    .await;
    assert_eq!(
        receiver
            .load_local_sync_profile("device-b")
            .await
            .unwrap()
            .unwrap()
            .revision,
        1
    );

    let server = Arc::new(SyncServer::new(
        source.clone() as Arc<dyn SyncRuntimeStorage>
    ));
    let auth_secret = "policy-test-shared-secret";
    server.set_auth_secret(Some(auth_secret.into())).await;
    let port = pick_port().await;
    server
        .start_with_addr(
            "policy-transfer-test",
            "device-a",
            Some("Device A"),
            Some(vec![format!("127.0.0.1:{port}")]),
            &format!("127.0.0.1:{port}"),
        )
        .await
        .unwrap();
    let client = client_for(&receiver, port, auth_secret);
    client.start();

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if data_platform::load_accepted_peer_sync_policy(
                &source_conn.lock().unwrap(),
                "device-b",
            )
            .unwrap()
            .is_some_and(|policy| policy.revision == 1)
                && server.get_connected_peer_entries().await.len() == 1
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("receiver's excluded policy should be accepted before data sync");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        stored_book_title(&receiver_conn.lock().unwrap()),
        "Local copy before narrowing"
    );

    source
        .apply_entity(&book_entity("Update while excluded", 2))
        .await
        .unwrap();
    server.broadcast_live_change(None).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        stored_book_title(&receiver_conn.lock().unwrap()),
        "Local copy before narrowing",
        "an excluded update must not overwrite the receiver's local row"
    );

    data_platform::upsert_sync_profile(
        &receiver_conn.lock().unwrap(),
        "device-b",
        "profile-device-b",
        "Device sync selection",
        true,
        Some(1),
        &[policy_rule(SyncMode::Full)],
    )
    .unwrap();
    client.stop_and_join().await;
    let widened_client = client_for(&receiver, port, auth_secret);
    widened_client.start();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if source
                .load_accepted_peer_sync_policy("device-b")
                .await
                .unwrap()
                .is_some_and(|policy| policy.revision == 2)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("sender should accept the widened receiver policy");
    let sender_page = source
        .load_entities_page_for_peer(
            "device-b",
            SyncReadClass::SyncLive,
            &Default::default(),
            0,
            8,
        )
        .await
        .unwrap();
    assert!(
        sender_page.iter().any(|entity| entity.id == "book-1"),
        "the widened peer projection should include the book"
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if stored_book_title(&receiver_conn.lock().unwrap()) == "Update while excluded" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("widening followed by reconnect should backfill current source state");
    widened_client.stop_and_join().await;
    server.stop().await;
}
