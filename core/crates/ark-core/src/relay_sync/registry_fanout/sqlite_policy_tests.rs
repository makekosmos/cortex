use super::sqlite_test_support::*;
use crate::application::sync_storage::PeerProgressPort;
use crate::data_platform::SyncMode;
use crate::replication_storage::PeerIdentityBackend;
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
    assert_ne!(
        peer_a
            .storage
            .load_version_vector()
            .await
            .expect("read A's backfilled task cursor")
            .get("task-B"),
        Some(&stale_b_cursor),
        "reconnection must advance A beyond its stale task cursor"
    );
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
