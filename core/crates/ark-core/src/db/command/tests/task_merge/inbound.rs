use super::*;

#[path = "inbound/rollback.rs"]
mod rollback;

async fn apply_batch(
    handle: &DbHandle,
    peer_id: &str,
    local_device_id: &str,
    identity: ark_domain::sync_policy::PolicyIdentity,
    batch: ark_domain::task_merge_sync::TaskMergeBatch,
) -> Result<ark_domain::task_merge_sync::TaskMergeBatchAck, DbError> {
    handle
        .apply_task_merge_batch(
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

async fn seed_project_link_target(path: &str) {
    ark_store::test_support::TaskMergeInboundFixture::open(std::path::Path::new(path))
        .unwrap()
        .seed_object(&crate::types::ArkObject {
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
        })
        .unwrap();
}

fn inspect_inbound(
    path: &str,
    peer_id: &str,
    operation_id: &str,
) -> ark_store::test_support::TaskMergeInboundState {
    ark_store::test_support::TaskMergeInboundFixture::open(std::path::Path::new(path))
        .unwrap()
        .inspect("task-1", peer_id, operation_id)
        .unwrap()
}

#[tokio::test]
async fn two_sqlite_peers_converge_on_the_same_conflict_set() {
    let (_left_dir, _left_path, left, mut left_owner) = open_merge_fixture().await;
    let right_dir = crate::test_support::temp_dir();
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
            .enroll_paired_device(
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
        .prepare_task_merge_batch(
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
        .query_with_store(
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
    let (_directory, path, handle, mut owner) = open_merge_fixture().await;
    seed_project_link_target(&path).await;
    let peer_id = "bootstrap-peer";
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .enroll_paired_device(
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
    let batch = ark_domain::task_merge_sync::TaskMergeBatch {
        batch_id: "b1".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 2,
        entries: vec![
            ark_domain::task_merge_sync::TaskMergeJournalEntry {
                position: 1,
                record: ark_domain::task_merge_sync::TaskMergeJournalRecord::Bootstrap(bootstrap),
            },
            ark_domain::task_merge_sync::TaskMergeJournalEntry {
                position: 2,
                record: ark_domain::task_merge_sync::TaskMergeJournalRecord::Operation {
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

    let state = inspect_inbound(&path, peer_id, "");
    assert_eq!(state.object.unwrap().title, "edited after bootstrap");
    assert_eq!(
        state
            .links
            .iter()
            .map(|link| link.id.as_str())
            .collect::<Vec<_>>(),
        ["task-project-link"]
    );
    assert_eq!(
        (
            state.bootstrap_count,
            state.operation_count,
            state.journal.len()
        ),
        (1, 11, 2)
    );
    assert!(matches!(
        state.journal[0].record,
        ark_domain::task_merge_sync::TaskMergeJournalRecord::Bootstrap(_)
    ));
    assert!(matches!(
        state.journal[1].record,
        ark_domain::task_merge_sync::TaskMergeJournalRecord::Operation { .. }
    ));
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn operation_before_bootstrap_stays_unpublished_until_bootstrap_arrives() {
    let (_directory, path, handle, mut owner) = open_merge_fixture().await;
    seed_project_link_target(&path).await;
    let peer_id = "pending-bootstrap-peer";
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();
    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let mut edit = operation('f', &[], "edited after bootstrap");
    edit.parents = vec![bootstrap().baseline_ids[0].1.clone()];
    let operation_batch = ark_domain::task_merge_sync::TaskMergeBatch {
        batch_id: "c1".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 1,
        entries: vec![ark_domain::task_merge_sync::TaskMergeJournalEntry {
            position: 1,
            record: ark_domain::task_merge_sync::TaskMergeJournalRecord::Operation {
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
    let state = inspect_inbound(&path, peer_id, "");
    assert!(state.object.is_none());
    assert!(state.journal.is_empty());

    let bootstrap_record =
        ark_domain::task_merge_sync::TaskMergeJournalRecord::Bootstrap(bootstrap());
    let bootstrap_batch = ark_domain::task_merge_sync::TaskMergeBatch {
        batch_id: "c2".repeat(16),
        identity: identity.clone(),
        after_position: 1,
        through_position: 2,
        entries: vec![ark_domain::task_merge_sync::TaskMergeJournalEntry {
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
    let state = inspect_inbound(&path, peer_id, "");
    assert_eq!(state.object.unwrap().title, "edited after bootstrap");
    assert!(matches!(
        state.journal[0].record,
        ark_domain::task_merge_sync::TaskMergeJournalRecord::Bootstrap(_)
    ));
    assert!(matches!(
        state.journal[1].record,
        ark_domain::task_merge_sync::TaskMergeJournalRecord::Operation { .. }
    ));
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn inbound_batch_atomically_applies_remote_resolution_and_replays_idempotently() {
    let (_directory, path, handle, mut owner) = open_merge_fixture().await;
    let peer_id = "paired-peer";
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();
    let identity = ark_application::sync_policy::default_sync_policy_identity();
    let batch = ark_domain::task_merge_sync::TaskMergeBatch {
        batch_id: "a1".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 2,
        entries: vec![
            ark_domain::task_merge_sync::TaskMergeJournalEntry {
                position: 1,
                record: ark_domain::task_merge_sync::TaskMergeJournalRecord::Operation {
                    operation: operation('1', &[], "remote base"),
                    expected_conflict_digest: None,
                },
            },
            ark_domain::task_merge_sync::TaskMergeJournalEntry {
                position: 2,
                record: ark_domain::task_merge_sync::TaskMergeJournalRecord::Operation {
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
    altered.entries[1].record = ark_domain::task_merge_sync::TaskMergeJournalRecord::Operation {
        operation: operation('2', &['1'], "altered resolution"),
        expected_conflict_digest: Some(format!("sha256:{}", "ab".repeat(32))),
    };
    assert!(
        apply_batch(&handle, peer_id, "merge-test-device", identity, altered)
            .await
            .is_err()
    );
    let state = inspect_inbound(&path, peer_id, id('2').as_str());
    assert_eq!(state.operation_count, 2);
    assert!(state.journal.is_empty());
    assert_eq!(state.inbound_cursor, Some(2));
    assert_eq!(
        state.resolution_guard.as_deref(),
        Some(format!("sha256:{}", "ab".repeat(32)).as_str())
    );
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn excluded_local_task_category_rejects_incoming_batch_without_advancing_cursor() {
    let (_directory, path, handle, mut owner) = open_merge_fixture().await;
    let peer_id = "paired-peer";
    let endpoint = ark_net::iroh::SecretKey::generate().public().to_string();
    handle
        .enroll_paired_device(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("merge-test-device".into(), peer_id.into(), endpoint),
        )
        .await
        .unwrap();
    let profile = handle
        .save_local_sync_profile(
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
    let identity = ark_sync::protocol::sync_policy_identity(Some(&profile));
    let batch = ark_domain::task_merge_sync::TaskMergeBatch {
        batch_id: "bb".repeat(16),
        identity: identity.clone(),
        after_position: 0,
        through_position: 1,
        entries: vec![ark_domain::task_merge_sync::TaskMergeJournalEntry {
            position: 1,
            record: ark_domain::task_merge_sync::TaskMergeJournalRecord::Operation {
                operation: operation('1', &[], "private on this device"),
                expected_conflict_digest: None,
            },
        }],
    };
    assert!(matches!(
        apply_batch(&handle, peer_id, "merge-test-device", identity, batch).await,
        Err(DbError::Permission(_))
    ));
    let state = inspect_inbound(&path, peer_id, "");
    assert_eq!(state.operation_count, 0);
    assert_eq!(state.inbound_progress_count, 0);
    shutdown(&mut owner).await;
}
