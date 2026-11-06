use crate::object_mutation::{
    build_local_object_delete_commit, plan_local_object_delete, LocalObjectDelete,
    LocalObjectDeleteFacts, ObjectMutationError,
};
use ark_domain::object_write::ObjectWriteSnapshot;

fn snapshot(revision: &str) -> ObjectWriteSnapshot {
    ObjectWriteSnapshot {
        exists: true,
        type_id: Some("core.note".into()),
        type_version: Some("0.1.0".into()),
        revision: Some(revision.into()),
    }
}

#[test]
fn delete_plan_requires_the_expected_snapshot_and_builds_sync_commit() {
    let write = LocalObjectDelete {
        id: "note-1".into(),
        expected_snapshot: Some(snapshot("revision-1")),
        device_id: "device-1".into(),
    };
    let facts = LocalObjectDeleteFacts {
        current_snapshot: snapshot("revision-2"),
        type_id: Some("core.note".into()),
    };
    assert_eq!(
        plan_local_object_delete(write.clone(), facts),
        Err(ObjectMutationError::StaleSnapshot)
    );

    let plan = plan_local_object_delete(
        write,
        LocalObjectDeleteFacts {
            current_snapshot: snapshot("revision-1"),
            type_id: Some("core.note".into()),
        },
    )
    .expect("current snapshot creates a delete plan");
    let commit = build_local_object_delete_commit(plan, "time:1:device-1".into());
    assert_eq!(commit.id, "note-1");
    assert_eq!(commit.entity.data["typeId"], "core.note");
    assert_eq!(commit.entity.deleted, Some(true));
}
