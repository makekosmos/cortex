use super::*;

fn write_backup(state: &Arc<ServiceState>, db_path: &str, backup_id: &str) {
    let directory = db::backups_dir(db_path);
    std::fs::create_dir_all(&directory).unwrap();
    let destination = directory.join(backup_id);
    with_conn(state, |connection| {
        db::backup_to_file(connection, destination.to_str().unwrap())
    })
    .unwrap();
}

async fn upsert_note(state: &Arc<ServiceState>, id: &str, title: &str) {
    handle_request(
        state,
        Request::UpsertObject {
            object: ArkObjectWrite {
                id: id.into(),
                type_id: "com.kosmos.note".into(),
                type_version: Some("1.0.0".into()),
                title: title.into(),
                content_json: json!({"type": "doc", "content": []}),
                props_json: json!({"description": null, "extensions": {}}),
                created_at: "2026-09-15T00:00:00.000Z".into(),
                updated_at: "2026-09-15T00:00:00.000Z".into(),
                deleted_at: None,
            },
            expected_snapshot: None,
            operation_id: None,
            device_id: Some("device-a".into()),
        },
    )
    .await
    .expect("commit note through Engine");
}

async fn note_title(state: &Arc<ServiceState>, id: &str) -> Option<String> {
    handle_request(state, Request::GetObject { id: id.into() })
        .await
        .expect("read note through Engine")["title"]
        .as_str()
        .map(str::to_owned)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stale_restore_is_refused_after_existing_iroh_peer_received_change() {
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
    let backup_id = "ark.db.backup-before-peer-change";
    write_backup(&state_a, &db_path, backup_id);
    upsert_note(&state_a, "peer-visible-note", "Peer-visible change").await;
    wait_until("B receives A's committed note", async || {
        note_title(&state_b, "peer-visible-note").await.as_deref() == Some("Peer-visible change")
    })
    .await;

    let error = handle_request(
        &state_a,
        Request::DbBackupRestore {
            backup_id: backup_id.into(),
        },
    )
    .await
    .expect_err("a backup older than a peer-visible commit must be refused");
    assert!(error.contains("stale snapshot predates committed sync history"));
    assert!(error.contains("without changing the live database"));
    assert_eq!(
        note_title(&state_a, "peer-visible-note").await.as_deref(),
        Some("Peer-visible change")
    );
    assert_eq!(
        note_title(&state_b, "peer-visible-note").await.as_deref(),
        Some("Peer-visible change")
    );

    assert_eq!(
        snapshot(&state_a).await["running"],
        json!(true),
        "safe refusal must restart A's sync runtime"
    );
    assert_eq!(
        snapshot(&state_b).await["running"],
        json!(true),
        "restore refusal must leave B's sync runtime running"
    );

    handle_request(&state_a, Request::StopSync).await.unwrap();
    handle_request(&state_b, Request::StopSync).await.unwrap();
}
