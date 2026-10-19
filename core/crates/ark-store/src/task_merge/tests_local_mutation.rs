use super::{bootstrap, connection};
use crate::task_merge::load_local_mutation_state;
use rusqlite::params;

#[test]
fn local_mutation_state_loads_projection_links_and_legacy_revision() {
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
    connection
        .execute(
            "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 0)",
            params![task.id, "2026-10-17T00:00:00.000Z:000001:device-a"],
        )
        .expect("legacy revision persists");

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
    assert_eq!(
        state.snapshot.revision.as_deref(),
        state.legacy_revision.as_deref()
    );
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
