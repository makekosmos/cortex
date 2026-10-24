use super::{bootstrap, connection};
use crate::task_merge::{
    check_bootstrap, load_local_mutation_state, materialize_task_if_ready, persist_bootstrap,
    TaskMergeBootstrapCheck, TaskMergeProjectionOutcome,
};
use ark_domain::task_merge::TaskMergeUnit;
use rusqlite::params;

#[test]
fn local_mutation_state_loads_projection_and_links() {
    let connection = connection();
    let task = bootstrap().object;
    crate::objects::upsert_object(&connection, &task).expect("task projection persists");
    connection
        .execute(
            "INSERT INTO object_links(id, source_object_id, target_object_id, link_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["link-z", task.id, "target", "task.project", "2026-10-17T00:00:00Z"],
        )
        .expect("first source link persists");
    connection
        .execute(
            "INSERT INTO object_links(id, source_object_id, target_object_id, link_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["link-a", task.id, "target", "task.project", "2026-10-17T00:00:00Z"],
        )
        .expect("second source link persists");

    let state = load_local_mutation_state(&connection, &task.id)
        .expect("local task state loads from Store");

    assert_eq!(state.object, Some(task.clone()));
    assert!(!state.has_bootstrap);
    assert_eq!(
        state
            .source_links
            .iter()
            .map(|link| link.id.as_str())
            .collect::<Vec<_>>(),
        ["link-a", "link-z"]
    );
    assert_eq!(state.snapshot.revision, None);
    assert_eq!(state.snapshot.type_id.as_deref(), Some("core.task"));
    assert_eq!(state.snapshot.type_version.as_deref(), Some("0.1.0"));
    assert!(state.snapshot.exists);
}

#[test]
fn task_snapshot_token_is_absent_until_bootstrap_exists() {
    let connection = connection();

    assert_eq!(
        crate::task_merge::task_snapshot_token(&connection, "task-1")
            .expect("unbootstrapped tasks have no merge token"),
        None
    );
}

#[test]
fn store_materializes_bootstrap_projection_and_links() {
    let connection = connection();
    let bootstrap = bootstrap();
    let prepared = match check_bootstrap(&connection, &bootstrap).expect("bootstrap check succeeds")
    {
        TaskMergeBootstrapCheck::New(prepared) => prepared,
        TaskMergeBootstrapCheck::Replay => panic!("fixture bootstrap is new"),
    };
    persist_bootstrap(&connection, prepared).expect("bootstrap persists");

    let state = load_local_mutation_state(&connection, &bootstrap.object.id)
        .expect("Store loads the causal snapshot for local mutation planning");
    assert_eq!(state.unit_conflicts.len(), TaskMergeUnit::ALL.len());
    assert!(state
        .unit_conflicts
        .iter()
        .all(|(_, conflict)| { conflict.variants.len() == 1 }));
    assert!(state.snapshot.revision.is_some());

    let outcome = materialize_task_if_ready(&connection, &bootstrap.object.id)
        .expect("Store materializes a complete task history");

    assert_eq!(
        outcome,
        TaskMergeProjectionOutcome::Materialized {
            object: Some(Box::new(bootstrap.object.clone())),
            hard_deleted: false,
        }
    );
    assert_eq!(
        crate::objects::get_object(&connection, &bootstrap.object.id)
            .expect("materialized object reads"),
        Some(bootstrap.object)
    );
    assert_eq!(
        crate::objects::list_object_links(&connection).expect("materialized links read"),
        bootstrap.links
    );
}
