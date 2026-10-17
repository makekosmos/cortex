use super::*;

async fn apply_batch(
    handle: &DbHandle,
    peer_id: &str,
    local_device_id: &str,
    identity: ark_application::sync_storage::PolicyIdentity,
    batch: ark_application::sync_storage::TaskMergeBatch,
) -> Result<ark_application::sync_storage::TaskMergeBatchAck, DbError> {
    handle
        .submit(
            CommandContext::authenticated_peer(peer_id.into()),
            ApplyTaskMergeBatchCommand::new(
                peer_id.into(),
                local_device_id.into(),
                identity,
                batch,
            ),
        )
        .await
}

fn bootstrap() -> ark_domain::task_merge::TaskMergeBootstrap {
    use ark_domain::task_merge::{OperationId, TaskMergeBootstrap, TaskMergeUnit};

    TaskMergeBootstrap {
        id: OperationId::parse("10000000000000000000000000000000").unwrap(),
        object: crate::types::ArkObject {
            id: "task-1".into(),
            type_id: "core.task".into(),
            type_version: "0.1.0".into(),
            title: "from bootstrap".into(),
            content_json: serde_json::json!({"type":"doc","content":[]}),
            props_json: serde_json::json!({
                "state":{"kind":"open","at":null},
                "planning":{"kind":"inbox","period":"any","date":null},
                "deadline":null,"reminders":[],"recurrence":null,
                "priority":"none","checklist":[]
            }),
            created_at: "2026-10-17T00:00:00Z".into(),
            updated_at: "2026-10-17T00:00:00Z".into(),
            deleted_at: None,
        },
        links: vec![crate::types::ObjectLink {
            id: "task-project-link".into(),
            source_object_id: "task-1".into(),
            target_object_id: "project-1".into(),
            link_type: "task.project".into(),
            created_at: "2026-10-17T00:00:00Z".into(),
        }],
        source_hlc: "2026-10-17T00:00:00.000Z:000001:source-device".into(),
        baseline_ids: TaskMergeUnit::ALL
            .iter()
            .enumerate()
            .map(|(index, unit)| {
                (
                    *unit,
                    OperationId::parse(format!("{:032x}", index + 1)).unwrap(),
                )
            })
            .collect(),
    }
}

async fn seed_project_link_target(handle: &DbHandle) {
    handle
        .with_connection(|connection| {
            crate::db::upsert_object(
                connection,
                &crate::types::ArkObject {
                    id: "project-1".into(),
                    type_id: "core.task".into(),
                    type_version: "0.1.0".into(),
                    title: "Project".into(),
                    content_json: serde_json::json!({}),
                    props_json: serde_json::json!({
                        "state":{"kind":"open","at":null},
                        "planning":{"kind":"inbox","period":"any","date":null},
                        "deadline":null,"reminders":[],"recurrence":null,
                        "priority":"none","checklist":[]
                    }),
                    created_at: "2026-10-17T00:00:00Z".into(),
                    updated_at: "2026-10-17T00:00:00Z".into(),
                    deleted_at: None,
                },
            )
        })
        .unwrap();
}

#[tokio::test]
async fn two_sqlite_peers_converge_on_the_same_conflict_set() {
    let (_left_dir, _left_path, left, mut left_owner) = open_merge_fixture().await;
    let right_dir = tempfile::tempdir().unwrap();
    let right_path = right_dir
        .path()
        .join("task-merge-right.db")
        .to_string_lossy()
        .into_owned();
    let (right, mut right_owner) = DbHandle::open(right_path.clone(), "right-device".into())
        .await
        .unwrap();
    let peer_id = "merge-test-device";
    let left_endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    let right_endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    for (handle, local_id, remote_id, endpoint) in [
        (&left, "merge-test-device", "right-device", right_endpoint),
        (&right, "right-device", peer_id, left_endpoint),
    ] {
        handle
            .submit(
                CommandContext::engine(),
                EnrollPairedDeviceCommand::new(local_id.into(), remote_id.into(), endpoint),
            )
            .await
            .unwrap();
    }
    append(&left, operation('1', &[], "common base"), None)
        .await
        .unwrap();
    for (id, title) in [('2', "left edit"), ('3', "right edit")] {
        append(&left, operation(id, &['1'], title), None)
            .await
            .unwrap();
    }
    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let batch = left
        .submit(
            CommandContext::authenticated_peer("right-device".into()),
            PrepareTaskMergeBatchCommand::new("right-device".into(), identity.clone(), 100),
        )
        .await
        .unwrap()
        .unwrap();
    apply_batch(&right, peer_id, "merge-test-device", identity, batch)
        .await
        .unwrap();

    let conflict_set = right
        .query(
            CommandContext::engine(),
            TaskConflictSetQuery::new(
                "task-1".into(),
                ark_domain::task_merge::TaskMergeUnit::Title,
            ),
        )
        .await
        .unwrap()
        .conflict_set
        .unwrap();
    assert_eq!(conflict_set.common_ancestors.len(), 1);
    assert_eq!(conflict_set.variants.len(), 2);
    assert!(conflict_set.has_conflict());
    shutdown(&mut left_owner).await;
    shutdown(&mut right_owner).await;
}

#[tokio::test]
async fn bootstrap_and_followup_operation_materialize_atomically_and_replay() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    seed_project_link_target(&handle).await;
    let peer_id = "bootstrap-peer";
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();

    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let bootstrap = bootstrap();
    let title_baseline = bootstrap.baseline_ids[0].1.clone();
    let mut edit = operation('f', &[], "edited after bootstrap");
    edit.parents = vec![title_baseline];
    let batch = ark_application::sync_storage::TaskMergeBatch {
        batch_id: "b1".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 2,
        entries: vec![
            ark_application::sync_storage::TaskMergeJournalEntry {
                position: 1,
                record: ark_application::sync_storage::TaskMergeJournalRecord::Bootstrap(bootstrap),
            },
            ark_application::sync_storage::TaskMergeJournalEntry {
                position: 2,
                record: ark_application::sync_storage::TaskMergeJournalRecord::Operation {
                    operation: edit,
                    expected_conflict_digest: None,
                },
            },
        ],
    };

    let applied = apply_batch(
        &handle,
        peer_id,
        "merge-test-device",
        identity.clone(),
        batch.clone(),
    )
    .await
    .unwrap();
    assert!(!applied.replayed);
    let replayed = apply_batch(&handle, peer_id, "merge-test-device", identity, batch)
        .await
        .unwrap();
    assert!(replayed.replayed);

    handle
        .with_connection(|connection| {
            let object = crate::db::get_object(connection, "task-1")?.unwrap();
            assert_eq!(object.title, "edited after bootstrap");
            assert_eq!(
                crate::db::list_object_links(connection)?
                    .into_iter()
                    .filter(|link| link.source_object_id == "task-1")
                    .map(|link| link.id)
                    .collect::<Vec<_>>(),
                vec!["task-project-link"]
            );
            let (bootstraps, operations, journal): (i64, i64, i64) = connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM task_merge_bootstraps),
                        (SELECT COUNT(*) FROM task_merge_operations),
                        (SELECT COUNT(*) FROM task_merge_journal)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|error| error.to_string())?;
            assert_eq!((bootstraps, operations, journal), (1, 11, 2));
            let page = crate::db::task_merge::read_journal_page(connection, 0, 2, 10)
                .map_err(|error| error.to_string())?;
            assert!(matches!(
                page.entries[0].record,
                ark_application::sync_storage::TaskMergeJournalRecord::Bootstrap(_)
            ));
            assert!(matches!(
                page.entries[1].record,
                ark_application::sync_storage::TaskMergeJournalRecord::Operation { .. }
            ));
            Ok::<(), String>(())
        })
        .unwrap();
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn operation_before_bootstrap_stays_unpublished_until_bootstrap_arrives() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    seed_project_link_target(&handle).await;
    let peer_id = "pending-bootstrap-peer";
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();
    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let mut edit = operation('f', &[], "edited after bootstrap");
    edit.parents = vec![bootstrap().baseline_ids[0].1.clone()];
    let operation_batch = ark_application::sync_storage::TaskMergeBatch {
        batch_id: "c1".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 1,
        entries: vec![ark_application::sync_storage::TaskMergeJournalEntry {
            position: 1,
            record: ark_application::sync_storage::TaskMergeJournalRecord::Operation {
                operation: edit,
                expected_conflict_digest: None,
            },
        }],
    };
    apply_batch(
        &handle,
        peer_id,
        "merge-test-device",
        identity.clone(),
        operation_batch,
    )
    .await
    .unwrap();
    handle
        .with_connection(|connection| {
            assert!(crate::db::get_object(connection, "task-1")?.is_none());
            let journal_rows: i64 = connection
                .query_row("SELECT COUNT(*) FROM task_merge_journal", [], |row| {
                    row.get(0)
                })
                .map_err(|error| error.to_string())?;
            assert_eq!(journal_rows, 0);
            Ok::<(), String>(())
        })
        .unwrap();

    let bootstrap_record =
        ark_application::sync_storage::TaskMergeJournalRecord::Bootstrap(bootstrap());
    let bootstrap_batch = ark_application::sync_storage::TaskMergeBatch {
        batch_id: "c2".repeat(16),
        identity: identity.clone(),
        after_position: 1,
        through_position: 2,
        entries: vec![ark_application::sync_storage::TaskMergeJournalEntry {
            position: 2,
            record: bootstrap_record,
        }],
    };
    apply_batch(
        &handle,
        peer_id,
        "merge-test-device",
        identity,
        bootstrap_batch,
    )
    .await
    .unwrap();
    handle
        .with_connection(|connection| {
            assert_eq!(
                crate::db::get_object(connection, "task-1")?.unwrap().title,
                "edited after bootstrap"
            );
            let page = crate::db::task_merge::read_journal_page(connection, 0, 2, 10)
                .map_err(|error| error.to_string())?;
            assert!(matches!(
                page.entries[0].record,
                ark_application::sync_storage::TaskMergeJournalRecord::Bootstrap(_)
            ));
            assert!(matches!(
                page.entries[1].record,
                ark_application::sync_storage::TaskMergeJournalRecord::Operation { .. }
            ));
            Ok::<(), String>(())
        })
        .unwrap();
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn inbound_batch_atomically_applies_remote_resolution_and_replays_idempotently() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    let peer_id = "paired-peer";
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();
    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let batch = ark_application::sync_storage::TaskMergeBatch {
        batch_id: "a1".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 2,
        entries: vec![
            ark_application::sync_storage::TaskMergeJournalEntry {
                position: 1,
                record: ark_application::sync_storage::TaskMergeJournalRecord::Operation {
                    operation: operation('1', &[], "remote base"),
                    expected_conflict_digest: None,
                },
            },
            ark_application::sync_storage::TaskMergeJournalEntry {
                position: 2,
                record: ark_application::sync_storage::TaskMergeJournalRecord::Operation {
                    operation: operation('2', &['1'], "remote resolution"),
                    expected_conflict_digest: Some(format!("sha256:{}", "ab".repeat(32))),
                },
            },
        ],
    };

    let applied = apply_batch(
        &handle,
        peer_id,
        "merge-test-device",
        identity.clone(),
        batch.clone(),
    )
    .await
    .unwrap();
    assert!(!applied.replayed);
    assert_eq!(applied.through_position, 2);
    let replayed = apply_batch(
        &handle,
        peer_id,
        "merge-test-device",
        identity.clone(),
        batch.clone(),
    )
    .await
    .unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.through_position, 2);

    let mut altered = batch.clone();
    altered.entries[1].record = ark_application::sync_storage::TaskMergeJournalRecord::Operation {
        operation: operation('2', &['1'], "altered resolution"),
        expected_conflict_digest: Some(format!("sha256:{}", "ab".repeat(32))),
    };
    assert!(
        apply_batch(&handle, peer_id, "merge-test-device", identity, altered)
            .await
            .is_err()
    );
    let (operation_count, journal_count, cursor, guard): (i64, i64, i64, String) = handle
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT
                        (SELECT COUNT(*) FROM task_merge_operations),
                        (SELECT COUNT(*) FROM task_merge_journal),
                        (SELECT received_through FROM task_merge_inbound_progress
                            WHERE peer_device_id = ?1),
                        (SELECT expected_conflict_digest FROM task_merge_resolution_guards
                            WHERE operation_id = ?2)",
                    rusqlite::params![peer_id, id('2').as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap()
        });
    assert_eq!((operation_count, journal_count, cursor), (2, 0, 2));
    assert_eq!(guard, format!("sha256:{}", "ab".repeat(32)));
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn excluded_local_task_category_rejects_incoming_batch_without_advancing_cursor() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    let peer_id = "paired-peer";
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();
    let profile = handle
        .submit(
            CommandContext::engine(),
            SaveLocalSyncProfileCommand::new(
                "merge-test-device".into(),
                "phone".into(),
                "Phone".into(),
                true,
                None,
                vec![ark_application::sync_policy::SyncProfileRule {
                    resource_kind: "type".into(),
                    resource_id: "core.task".into(),
                    mode: ark_domain::selective_sync::SyncMode::None,
                    filter: ark_domain::selective_sync::FilterV1::match_all(),
                }],
            ),
        )
        .await
        .unwrap();
    let identity = crate::protocol::sync_policy_identity(Some(&profile));
    let batch = ark_application::sync_storage::TaskMergeBatch {
        batch_id: "bb".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 1,
        entries: vec![ark_application::sync_storage::TaskMergeJournalEntry {
            position: 1,
            record: ark_application::sync_storage::TaskMergeJournalRecord::Operation {
                operation: operation('1', &[], "private on this device"),
                expected_conflict_digest: None,
            },
        }],
    };
    assert!(matches!(
        apply_batch(&handle, peer_id, "merge-test-device", identity, batch).await,
        Err(DbError::Permission(_))
    ));
    let (operation_count, cursor_count): (i64, i64) = handle.with_connection(|connection| {
        connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM task_merge_operations),
                        (SELECT COUNT(*) FROM task_merge_inbound_progress)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    });
    assert_eq!((operation_count, cursor_count), (0, 0));
    shutdown(&mut owner).await;
}
