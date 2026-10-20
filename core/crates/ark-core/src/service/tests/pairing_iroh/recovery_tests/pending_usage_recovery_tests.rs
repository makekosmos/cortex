use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stale_restore_preserves_pending_usage_and_origin_sequence_before_peer_resync() {
    let (fixture_a, fixture_b, _) = pending_pairing().await;
    let state_a = fixture_a.state.clone();
    let state_b = fixture_b.state.clone();
    handle_request(
        &state_a,
        Request::AcceptPairing {
            device_id: "device-b".into(),
        },
    )
    .await
    .expect("accept existing Iroh peer");
    wait_until("both recovery-test devices online", async || {
        peer_online(&snapshot(&state_a).await, "device-b")
            && peer_online(&snapshot(&state_b).await, "device-a")
    })
    .await;

    let db_path = fixture_a.db_path().to_string_lossy().into_owned();
    let backup_id = "ark.db.backup-before-pending-usage";
    write_backup(&state_a, &db_path, backup_id);

    handle_request(&state_b, Request::StopSync)
        .await
        .expect("stop B so the next local usage change remains pending");
    seed_usage_sources(&state_a).await;
    upsert_usage_event(
        &state_a,
        "recovery-pending-usage-1",
        "2026-10-17T00:00:01.000Z",
    )
    .await;
    upsert_note(
        &state_a,
        "recovery-pending-note-1",
        "Pending before restore",
    )
    .await;
    let first = usage_event_state(&fixture_a.dir, "recovery-pending-usage-1");
    let first_sequence = first.sequence.expect("local usage has an origin sequence");
    assert!(first_sequence > 0);
    assert!(first.exists);

    let error = handle_request(
        &state_a,
        Request::DbBackupRestore {
            backup_id: backup_id.into(),
        },
    )
    .await
    .expect_err("restore must reject a snapshot older than a pending local commit");
    assert!(
        error.contains("stale snapshot predates committed sync history"),
        "restore refusal should identify the stale committed-feed snapshot; got: {error}"
    );
    assert!(usage_event_state(&fixture_a.dir, "recovery-pending-usage-1").exists);
    assert_eq!(
        usage_event_state(&fixture_a.dir, "recovery-pending-usage-1").sequence,
        Some(first_sequence)
    );

    upsert_usage_event(
        &state_a,
        "recovery-pending-usage-2",
        "2026-10-17T00:00:02.000Z",
    )
    .await;
    upsert_note(
        &state_a,
        "recovery-pending-note-2",
        "Pending after restore refusal",
    )
    .await;
    let second_sequence = usage_event_state(&fixture_a.dir, "recovery-pending-usage-2")
        .sequence
        .expect("second local usage has an origin sequence");
    assert!(
        second_sequence > first_sequence,
        "a post-refusal local commit must not reuse an origin sequence"
    );

    start_iroh(&state_b, "device-b", "Device B").await;
    let reconnect = handle_request(
        &state_b,
        Request::ConnectWithPairingCode {
            pairing_code: ticket_of(&state_a).await,
        },
    )
    .await
    .expect("B should reconnect to A's post-refusal endpoint");
    assert_eq!(reconnect["status"], json!("pending"));
    wait_until("both recovery-test devices reconnect", async || {
        peer_online(&snapshot(&state_a).await, "device-b")
            && peer_online(&snapshot(&state_b).await, "device-a")
    })
    .await;
    wait_until("pending object changes resync to B", async || {
        note_title(&state_b, "recovery-pending-note-1")
            .await
            .as_deref()
            == Some("Pending before restore")
            && note_title(&state_b, "recovery-pending-note-2")
                .await
                .as_deref()
                == Some("Pending after restore refusal")
    })
    .await;

    handle_request(&state_a, Request::StopSync).await.unwrap();
    handle_request(&state_b, Request::StopSync).await.unwrap();
}
