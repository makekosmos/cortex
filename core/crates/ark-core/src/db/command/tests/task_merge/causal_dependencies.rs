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
