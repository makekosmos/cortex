use super::*;

#[tokio::test]
async fn cross_unit_observed_head_stays_pending_until_it_arrives() {
    let (_directory, _path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "base"), None)
        .await
        .unwrap();

    let mut edit = operation('2', &['1'], "edit observed elsewhere");
    edit.observed_heads.push(id('4'));
    let pending = append(&handle, edit, None).await.unwrap();
    assert!(pending.dependencies_pending);

    let state_operation = TaskMergeOperation {
        id: id('4'),
        object_id: "task-1".into(),
        unit: TaskMergeUnit::State,
        parents: vec![],
        observed_heads: vec![id('1')],
        hlc: None,
        value: TaskMergeValue::Field(serde_json::json!({
            "kind": "open",
            "at": null
        })),
    };
    let resolved = append(&handle, state_operation, None).await.unwrap();
    assert!(!resolved.dependencies_pending);

    let title = read(&handle).await;
    assert!(!title.dependencies_pending);
    assert_eq!(title.conflict_set.unwrap().variants.len(), 1);
    shutdown(&mut owner).await;
}

#[tokio::test]
async fn sqlite_history_preserves_a_concurrent_delete_and_field_edit() {
    let (_directory, path, handle, mut owner) = open_merge_fixture().await;
    append(&handle, operation('1', &[], "before"), None)
        .await
        .unwrap();
    let existence_base = TaskMergeOperation {
        id: id('2'),
        object_id: "task-1".into(),
        unit: TaskMergeUnit::ObjectExistence,
        parents: vec![],
        observed_heads: vec![id('1')],
        hlc: None,
        value: TaskMergeValue::ObjectExists,
    };
    append(&handle, existence_base, None).await.unwrap();

    let deletion = TaskMergeOperation {
        id: id('3'),
        object_id: "task-1".into(),
        unit: TaskMergeUnit::ObjectExistence,
        parents: vec![id('2')],
        observed_heads: vec![id('1'), id('2')],
        hlc: None,
        value: TaskMergeValue::ObjectDeleted {
            deleted_at: "2026-10-17T00:00:00Z".into(),
        },
    };
    let edit = TaskMergeOperation {
        id: id('4'),
        object_id: "task-1".into(),
        unit: TaskMergeUnit::Title,
        parents: vec![id('1')],
        observed_heads: vec![id('1'), id('2')],
        hlc: None,
        value: TaskMergeValue::Field(serde_json::json!("edited concurrently")),
    };
    append(&handle, deletion.clone(), None).await.unwrap();
    append(&handle, edit.clone(), None).await.unwrap();

    let persisted = inspect(&path);
    let conflicts = ark_domain::task_merge::lifecycle_conflicts(&persisted.operations).unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].deletion, deletion);
    assert_eq!(conflicts[0].concurrent_head, edit);
    shutdown(&mut owner).await;
}
