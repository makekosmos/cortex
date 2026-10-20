use super::sqlite_test_support::*;
use crate::db::{CommandContext, ObjectReadQuery, SaveLocalSyncProfileCommand};
use crate::replication_storage::PeerIdentityBackend;
use crate::types::{ArkObject, SyncEntity};
use ark_application::sync_policy::SyncProfileRule;
use ark_application::sync_storage::{PeerProgressPort, SyncEntityMutationPort};
use ark_domain::hlc::HLC;
use ark_domain::selective_sync::FilterV1;
use ark_domain::selective_sync::SyncMode;
use std::time::{Duration, Instant};

#[tokio::test(flavor = "multi_thread")]
async fn actor_backed_iroh_sync_respects_task_policy_and_backfills_after_widening() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let path_a = directory.path().join("policy-device-a.sqlite");
    let path_b = directory.path().join("policy-device-b.sqlite");
    let path_c = directory.path().join("policy-device-c.sqlite");
    let mut peer_a = open_peer(&path_a, "device-A").await;
    let mut peer_b = open_peer(&path_b, "device-B").await;
    let mut peer_c = open_peer(&path_c, "device-C").await;

    set_task_policy(&peer_a, SyncMode::Full, None).await;
    set_task_policy(&peer_b, SyncMode::None, None).await;
    set_task_policy(&peer_c, SyncMode::Full, None).await;
    write_task(&peer_a, "task-A", "A current task").await;
    write_task(&peer_b, "task-B", "B original task").await;

    let endpoint_a = peer_a.secret.public();
    let endpoint_b = peer_b.secret.public();
    let endpoint_c = peer_c.secret.public();
    for (storage, local_id, peer_id, endpoint) in [
        (&peer_a.storage, "device-A", "device-B", endpoint_b),
        (&peer_a.storage, "device-A", "device-C", endpoint_c),
        (&peer_b.storage, "device-B", "device-A", endpoint_a),
        (&peer_c.storage, "device-C", "device-A", endpoint_a),
    ] {
        storage
            .enroll_paired_device(local_id, peer_id, &endpoint.to_string())
            .await
            .expect("enroll Iroh peer");
    }

    let (transport_a, relay_a) = start_policy_relay(&peer_a, "device-A", None);
    relay_a.start().await.expect("start device A");
    let (transport_c, relay_c) =
        start_policy_relay(&peer_c, "device-C", transport_a.endpoint_addr());
    relay_c.start().await.expect("start device C");
    let (transport_b, relay_b) =
        start_policy_relay(&peer_b, "device-B", transport_a.endpoint_addr());
    relay_b.start().await.expect("start device B");

    wait_until("initial registry admission", || async {
        transport_a.registry().is_data_ready_endpoint(&endpoint_b)
            && transport_a.registry().is_data_ready_endpoint(&endpoint_c)
            && transport_b.registry().is_data_ready_endpoint(&endpoint_a)
            && transport_c.registry().is_data_ready_endpoint(&endpoint_a)
    })
    .await;
    wait_for_accepted_task_mode(&peer_a, "device-B", 1, SyncMode::None).await;
    wait_for_accepted_task_mode(&peer_a, "device-C", 1, SyncMode::Full).await;
    wait_for_accepted_task_mode(&peer_b, "device-A", 1, SyncMode::Full).await;
    wait_for_accepted_task_mode(&peer_c, "device-A", 1, SyncMode::Full).await;

    wait_until("C receives A's task", || async {
        task_title(&peer_c, "task-A").await.as_deref() == Some("A current task")
    })
    .await;
    wait_until("A receives B's original task", || async {
        task_title(&peer_a, "task-B").await.as_deref() == Some("B original task")
    })
    .await;
    wait_until("A records B's original task cursor", || async {
        peer_a
            .storage
            .load_version_vector()
            .await
            .is_ok_and(|vector| vector.contains_key("task-B"))
    })
    .await;
    let stale_b_cursor = peer_a
        .storage
        .load_version_vector()
        .await
        .expect("read A's existing B task cursor")
        .get("task-B")
        .cloned()
        .expect("B task has an existing sync cursor");
    assert_eq!(task_title(&peer_b, "task-A").await, None);
    assert_eq!(
        task_title(&peer_b, "task-B").await.as_deref(),
        Some("B original task"),
        "excluding a type from sync must preserve B's local task"
    );

    // A stops accepting tasks. Reconnect B so it receives A's new receiver
    // policy before making a change that must remain local on B.
    set_task_policy(&peer_a, SyncMode::None, Some(1)).await;
    relay_b.stop().await;
    peer_b
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down B before policy refresh");
    drop(relay_b);
    drop(transport_b);
    drop(peer_b);
    wait_until("A observes B disconnect before policy refresh", || async {
        !transport_a.registry().is_data_ready_endpoint(&endpoint_b)
    })
    .await;
    peer_b = open_peer(&path_b, "device-B").await;
    let (transport_b, relay_b) =
        start_policy_relay(&peer_b, "device-B", transport_a.endpoint_addr());
    relay_b
        .start()
        .await
        .expect("reconnect B with its SQLite state");
    wait_until("B registry admission after policy refresh", || async {
        transport_a.registry().is_data_ready_endpoint(&endpoint_b)
            && transport_b.registry().is_data_ready_endpoint(&endpoint_a)
    })
    .await;
    wait_for_accepted_task_mode(&peer_b, "device-A", 2, SyncMode::None).await;
    write_task(&peer_b, "task-B", "B latest task").await;
    tokio::time::sleep(Duration::from_millis(750)).await;
    assert_eq!(
        task_title(&peer_a, "task-B").await.as_deref(),
        Some("B original task"),
        "A's excluded task policy must prevent B's later edit from arriving"
    );
    assert_eq!(
        peer_a
            .storage
            .load_version_vector()
            .await
            .expect("read A's task cursor while excluded")
            .get("task-B"),
        Some(&stale_b_cursor),
        "A's cursor must remain at the older task revision while policy excludes updates"
    );
    assert_eq!(
        task_title(&peer_b, "task-B").await.as_deref(),
        Some("B latest task"),
        "the excluded task remains available on B"
    );

    // Widen both receive policies while B is offline, then restart it. Its
    // existing version vector is deliberately stale for both task revisions.
    set_task_policy(&peer_a, SyncMode::Full, Some(2)).await;
    set_task_policy(&peer_b, SyncMode::Full, Some(1)).await;
    relay_b.stop().await;
    peer_b
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down B before widening-policy reconnect");
    drop(relay_b);
    drop(transport_b);
    drop(peer_b);
    wait_until("A observes B disconnect before widening", || async {
        !transport_a.registry().is_data_ready_endpoint(&endpoint_b)
    })
    .await;
    peer_b = open_peer(&path_b, "device-B").await;
    let (transport_b, relay_b) =
        start_policy_relay(&peer_b, "device-B", transport_a.endpoint_addr());
    relay_b
        .start()
        .await
        .expect("reconnect B with full task policy");

    wait_until("B registry admission after widening", || async {
        transport_a.registry().is_data_ready_endpoint(&endpoint_b)
            && transport_b.registry().is_data_ready_endpoint(&endpoint_a)
    })
    .await;
    wait_for_accepted_task_mode(&peer_b, "device-A", 3, SyncMode::Full).await;
    wait_for_accepted_task_mode(&peer_a, "device-B", 2, SyncMode::Full).await;
    wait_until("B receives A's current task after widening", || async {
        task_title(&peer_b, "task-A").await.as_deref() == Some("A current task")
    })
    .await;
    wait_until("A receives B's latest task after widening", || async {
        task_title(&peer_a, "task-B").await.as_deref() == Some("B latest task")
    })
    .await;
    // Merge-owned tasks advance on the causal-journal cursor, not the legacy
    // object version vector. The title assertion above proves the widened
    // policy backfilled B's latest causal operation.
    assert_eq!(
        task_title(&peer_c, "task-A").await.as_deref(),
        Some("A current task"),
        "C's full policy continues to expose A's task"
    );
    assert_eq!(
        task_title(&peer_b, "task-B").await.as_deref(),
        Some("B latest task"),
        "backfill must not replace B's own task with an old version"
    );

    relay_b.stop().await;
    relay_c.stop().await;
    relay_a.stop().await;
    peer_b
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down B actor");
    peer_a
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down A actor");
    peer_c
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down C actor");
}

fn note_rule(mode: SyncMode) -> SyncProfileRule {
    SyncProfileRule {
        resource_kind: "type".into(),
        // `note_obj` is a legacy alias; policy identities use the canonical type ID.
        resource_id: "com.kosmos.note".into(),
        mode,
        filter: FilterV1::match_all(),
    }
}

async fn register_note_type(peer: &ActorPeer) {
    let registrations = crate::canonical_types::definitions::canonical_type_registrations()
        .expect("load canonical type definitions");
    let registration = registrations
        .into_iter()
        .find(|registration| registration.type_id == "com.kosmos.note")
        .expect("canonical note type is defined");
    peer.db
        .submit_register_package_types(CommandContext::engine(), vec![registration])
        .await
        .expect("register note type before sync");
}

async fn set_note_policy(peer: &ActorPeer, mode: SyncMode, expected_revision: Option<i64>) {
    peer.db
        .submit(
            CommandContext::engine(),
            SaveLocalSyncProfileCommand::new(
                peer.storage.device_id(),
                "device-note-policy".into(),
                "Device note selection".into(),
                true,
                expected_revision,
                vec![note_rule(mode)],
            ),
        )
        .await
        .expect("save note sync policy");
}

async fn note_title(peer: &ActorPeer, id: &str) -> Option<String> {
    let value = peer
        .db
        .query_with_store(
            CommandContext::engine(),
            ObjectReadQuery::Get(id.to_owned()),
        )
        .await
        .expect("read note object");
    serde_json::from_value::<Option<ArkObject>>(value)
        .expect("decode note object")
        .map(|note| note.title)
}

fn pending_note_count(database: &std::path::Path, id: &str) -> i64 {
    let connection =
        rusqlite::Connection::open_with_flags(database, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open policy test database read-only");
    connection
        .query_row(
            "SELECT COUNT(*) FROM sync_pending_objects WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .expect("read pending note count")
}

async fn write_note(peer: &ActorPeer, id: &str, title: &str) {
    let hlc = HLC::now(&peer.storage.device_id()).to_string();
    let object = ArkObject {
        id: id.into(),
        type_id: "note_obj".into(),
        type_version: "0.0.0-legacy".into(),
        title: title.into(),
        content_json: serde_json::json!({
            "type": "doc",
            "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": title }] }]
        }),
        props_json: serde_json::json!({"description": title}),
        created_at: "2026-10-17T00:00:00.000Z".into(),
        updated_at: "2026-10-17T00:00:00.000Z".into(),
        deleted_at: None,
    };
    let mut data = serde_json::to_value(object)
        .expect("serialize note fixture")
        .as_object()
        .cloned()
        .expect("ArkObject serializes as an object");
    data.remove("id");
    peer.storage
        .apply_entity(&SyncEntity {
            entity_type: "object".into(),
            id: id.into(),
            data: data.clone(),
            hlc,
            deleted: None,
            origin_device_id: Some(peer.storage.device_id()),
            origin_seq: None,
        })
        .await
        .expect("persist note through the actor-backed sync adapter");
}

async fn wait_for_accepted_note_mode(
    peer: &ActorPeer,
    remote: &str,
    revision: i64,
    mode: SyncMode,
) {
    wait_until("accepted note policy", || async {
        peer.storage
            .load_accepted_peer_sync_policy(remote)
            .await
            .is_ok_and(|accepted| {
                accepted.is_some_and(|policy| {
                    policy.revision == revision
                        && policy.rules.iter().any(|rule| {
                            rule.resource_kind == "type"
                                && rule.resource_id == "com.kosmos.note"
                                && rule.mode == mode
                        })
                })
            })
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn actor_backed_iroh_note_policy_excludes_expands_and_shrinks_without_deleting() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let path_a = directory.path().join("note-policy-device-a.sqlite");
    let path_b = directory.path().join("note-policy-device-b.sqlite");
    let mut peer_a = open_peer(&path_a, "device-A").await;
    let mut peer_b = open_peer(&path_b, "device-B").await;
    register_note_type(&peer_a).await;
    register_note_type(&peer_b).await;
    set_note_policy(&peer_a, SyncMode::Full, None).await;
    set_note_policy(&peer_b, SyncMode::None, None).await;
    write_note(&peer_a, "note-A", "A original note").await;
    assert_eq!(
        note_title(&peer_a, "note-A").await.as_deref(),
        Some("A original note")
    );

    let endpoint_a = peer_a.secret.public();
    let endpoint_b = peer_b.secret.public();
    peer_a
        .storage
        .enroll_paired_device("device-A", "device-B", &endpoint_b.to_string())
        .await
        .expect("enroll B on A");
    peer_b
        .storage
        .enroll_paired_device("device-B", "device-A", &endpoint_a.to_string())
        .await
        .expect("enroll A on B");

    let (transport_a, relay_a) = start_policy_relay(&peer_a, "device-A", None);
    relay_a.start().await.expect("start device A");
    let (transport_b, relay_b) =
        start_policy_relay(&peer_b, "device-B", transport_a.endpoint_addr());
    relay_b.start().await.expect("start device B");
    wait_until("initial note registry admission", || async {
        transport_a.registry().is_data_ready_endpoint(&endpoint_b)
            && transport_b.registry().is_data_ready_endpoint(&endpoint_a)
    })
    .await;
    wait_for_accepted_note_mode(&peer_a, "device-B", 1, SyncMode::None).await;
    assert_eq!(pending_note_count(&path_b, "note-A"), 0);
    assert_eq!(
        note_title(&peer_b, "note-A").await,
        None,
        "excluded type must not create a remote copy"
    );

    // Widen B's receiver policy while it is offline. Its unchanged version
    // vector forces A to backfill the existing ordinary object on reconnect.
    set_note_policy(&peer_b, SyncMode::Full, Some(1)).await;
    relay_b.stop().await;
    peer_b
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down B before widening policy");
    drop(relay_b);
    drop(transport_b);
    drop(peer_b);
    wait_until("A observes B disconnect before widening", || async {
        !transport_a.registry().is_data_ready_endpoint(&endpoint_b)
    })
    .await;
    peer_b = open_peer(&path_b, "device-B").await;
    let (transport_b, relay_b) =
        start_policy_relay(&peer_b, "device-B", transport_a.endpoint_addr());
    relay_b
        .start()
        .await
        .expect("reconnect B with full note policy");
    wait_until("note policy registry admission after widening", || async {
        transport_a.registry().is_data_ready_endpoint(&endpoint_b)
            && transport_b.registry().is_data_ready_endpoint(&endpoint_a)
    })
    .await;
    wait_for_accepted_note_mode(&peer_a, "device-B", 2, SyncMode::Full).await;
    wait_until("B receives existing note after widening", || async {
        note_title(&peer_b, "note-A").await.as_deref() == Some("A original note")
    })
    .await;
    assert_eq!(
        pending_note_count(&path_b, "note-A"),
        0,
        "expanded policy must apply the existing note rather than leave it pending"
    );
    assert_eq!(
        note_title(&peer_b, "note-A").await.as_deref(),
        Some("A original note"),
        "expanded policy must deliver the existing note"
    );

    // Shrinking selection keeps the local object but prevents later revisions
    // of that type from reaching B.
    set_note_policy(&peer_b, SyncMode::None, Some(2)).await;
    relay_b.stop().await;
    peer_b
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down B before shrinking policy");
    drop(relay_b);
    drop(transport_b);
    drop(peer_b);
    wait_until("A observes B disconnect before shrinking", || async {
        !transport_a.registry().is_data_ready_endpoint(&endpoint_b)
    })
    .await;
    peer_b = open_peer(&path_b, "device-B").await;
    let (transport_b, relay_b) =
        start_policy_relay(&peer_b, "device-B", transport_a.endpoint_addr());
    relay_b
        .start()
        .await
        .expect("reconnect B with excluded note policy");
    wait_until("note policy registry admission after shrinking", || async {
        transport_a.registry().is_data_ready_endpoint(&endpoint_b)
            && transport_b.registry().is_data_ready_endpoint(&endpoint_a)
    })
    .await;
    wait_for_accepted_note_mode(&peer_a, "device-B", 3, SyncMode::None).await;
    write_note(&peer_a, "note-A", "A updated note").await;
    tokio::time::sleep(Duration::from_millis(750)).await;
    assert_eq!(pending_note_count(&path_b, "note-A"), 0);
    assert_eq!(
        note_title(&peer_b, "note-A").await.as_deref(),
        Some("A original note"),
        "shrinking policy must retain the local note and suppress the future revision"
    );

    relay_b.stop().await;
    relay_a.stop().await;
    peer_b
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down B actor");
    peer_a
        .owner
        .shutdown(Instant::now() + Duration::from_secs(5))
        .await
        .expect("shut down A actor");
}
