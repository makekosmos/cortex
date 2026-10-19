use super::{new_direct_client, pair_storages, pick_port, DirectClient, SyncServer};
use ark_application::sync_policy::SyncProfileRule;
use ark_application::sync_storage::{
    PeerProgressPort, SyncEntityMutationPort, SyncEntityReadPort, SyncReadClass,
};
use ark_core::replication_storage::PeerIdentityBackend;
use ark_core::sync_server::SyncRuntimeStorage;
use ark_core::types::{ArkObject, ObjectType, PeerRecord, SyncEntity};
use ark_core::{db, SqliteStorageBackend};
use ark_domain::selective_sync::{FilterV1, SyncMode};
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
        ark_store::data_platform::upsert_sync_profile(
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

fn stored_book_title_if_present(conn: &Connection) -> Option<String> {
    db::get_object(conn, "book-1")
        .unwrap()
        .map(|book| book.title)
}

fn client_for(
    storage: &Arc<SqliteStorageBackend>,
    port: u16,
    auth_secret: &str,
    device_id: &str,
) -> Arc<DirectClient> {
    Arc::new(new_direct_client(
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
        device_id.into(),
        format!("Device {device_id}"),
        "policy-transfer-test".into(),
        Vec::new(),
        Some(auth_secret.into()),
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn receiver_policy_stops_transfer_and_widening_resumes_after_reconnect() {
    let (source, source_endpoint, source_conn) = make_policy_storage("device-a", None);
    let (receiver, receiver_endpoint, receiver_conn) =
        make_policy_storage("device-b", Some(vec![policy_rule(SyncMode::Full)]));
    let (receiver_c, receiver_c_endpoint, receiver_c_conn) =
        make_policy_storage("device-c", Some(vec![policy_rule(SyncMode::Full)]));
    source
        .apply_entity(&book_entity("Original source version", 1))
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
    pair_storages(
        &source,
        "device-a",
        &source_endpoint,
        &receiver_c,
        "device-c",
        &receiver_c_endpoint,
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
    let client = client_for(&receiver, port, auth_secret, "device-b");
    let client_c = client_for(&receiver_c, port, auth_secret, "device-c");
    client.start();
    client_c.start();

    let policy_acceptance = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if ark_store::data_platform::load_accepted_peer_sync_policy(
                &source_conn.lock().unwrap(),
                "device-b",
            )
            .unwrap()
            .is_some_and(|policy| policy.revision == 1)
                && source
                    .load_accepted_peer_sync_policy("device-c")
                    .await
                    .unwrap()
                    .is_some_and(|policy| policy.revision == 1)
                && server.get_connected_peer_entries().await.len() == 2
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    if policy_acceptance.is_err() {
        let accepted_b = source
            .load_accepted_peer_sync_policy("device-b")
            .await
            .unwrap()
            .map(|policy| policy.revision);
        let accepted_c = source
            .load_accepted_peer_sync_policy("device-c")
            .await
            .unwrap()
            .map(|policy| policy.revision);
        let connected = server
            .get_connected_peer_entries()
            .await
            .into_iter()
            .map(|peer| peer.device_id)
            .collect::<Vec<_>>();
        panic!(
            "policy acceptance timeout: b={accepted_b:?}, c={accepted_c:?}, peers={connected:?}"
        );
    }
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if stored_book_title_if_present(&receiver_conn.lock().unwrap()).as_deref()
                == Some("Original source version")
                && stored_book_title_if_present(&receiver_c_conn.lock().unwrap()).as_deref()
                    == Some("Original source version")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("both opted-in devices should receive the initial book");
    assert_eq!(
        stored_book_title(&receiver_conn.lock().unwrap()),
        "Original source version"
    );
    assert_eq!(
        stored_book_title(&receiver_c_conn.lock().unwrap()),
        "Original source version",
        "a different device's full policy must remain independent"
    );

    ark_store::data_platform::upsert_sync_profile(
        &receiver_conn.lock().unwrap(),
        "device-b",
        "profile-device-b",
        "Device sync selection",
        true,
        Some(1),
        &[policy_rule(SyncMode::None)],
    )
    .unwrap();
    client.stop_and_join().await;
    let narrowed_client = client_for(&receiver, port, auth_secret, "device-b");
    narrowed_client.start();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if source
                .load_accepted_peer_sync_policy("device-b")
                .await
                .unwrap()
                .is_some_and(|policy| policy.revision == 2)
                && server.get_connected_peer_entries().await.len() == 2
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("sender should accept the narrowed receiver policy");
    assert_eq!(
        stored_book_title(&receiver_conn.lock().unwrap()),
        "Original source version",
        "narrowing a class stops transfer but preserves the received local row"
    );

    let excluded_update = book_entity("Update while excluded", 2);
    source.apply_entity(&excluded_update).await.unwrap();
    let mut source_vector = source.load_version_vector().await.unwrap();
    source_vector.insert(excluded_update.id.clone(), excluded_update.hlc.clone());
    source.store_version_vector(&source_vector).await.unwrap();
    server.broadcast_live_change(None).await;
    let receiver_c_after = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if stored_book_title(&receiver_c_conn.lock().unwrap()) == "Update while excluded" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    receiver_c_after.expect("another device's full policy should receive the live update");
    assert_eq!(
        stored_book_title(&receiver_conn.lock().unwrap()),
        "Original source version",
        "an excluded update must not overwrite the receiver's local row"
    );

    ark_store::data_platform::upsert_sync_profile(
        &receiver_conn.lock().unwrap(),
        "device-b",
        "profile-device-b",
        "Device sync selection",
        true,
        Some(2),
        &[policy_rule(SyncMode::Full)],
    )
    .unwrap();
    narrowed_client.stop_and_join().await;
    let widened_client = client_for(&receiver, port, auth_secret, "device-b");
    widened_client.start();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if source
                .load_accepted_peer_sync_policy("device-b")
                .await
                .unwrap()
                .is_some_and(|policy| policy.revision == 3)
                && server.is_connected_to("device-b").await
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
    let backfill = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if stored_book_title(&receiver_conn.lock().unwrap()) == "Update while excluded" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    if backfill.is_err() {
        let source_vector = source.load_version_vector().await.unwrap();
        let receiver_vector = receiver.load_version_vector().await.unwrap();
        let title = stored_book_title(&receiver_conn.lock().unwrap());
        let source_hlc = source_vector.get("book-1").cloned();
        let receiver_hlc = receiver_vector.get("book-1").cloned();
        let connected = server.is_connected_to("device-b").await;
        panic!(
            "backfill timeout: title={title:?}, source={source_hlc:?}, \
             receiver={receiver_hlc:?}, connected={connected}"
        );
    }
    widened_client.stop_and_join().await;
    client_c.stop_and_join().await;
    server.stop().await;
}
