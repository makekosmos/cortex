use super::{
    canonical_operation, check_bootstrap, check_operation, materialize_task_if_ready,
    persist_bootstrap, persist_operation, storage_error, TaskMergeBootstrapCheck,
    TaskMergeOperationCheck, TaskMergeOperationOrigin, TaskMergeProjectionOutcome,
    TaskMergeStoreError,
};
use crate::sync_versions::{bump_local_revision, LocalRevisionTarget};
use ark_domain::task_merge::{TaskMergeBootstrap, TaskMergeOperation};
use ark_domain::types::{ArkObject, ObjectLink};
use rusqlite::Connection;

#[derive(Debug, Clone, PartialEq)]
pub struct TaskMergeLocalMutationPlan {
    pub expected_state: super::TaskMergeLocalMutationState,
    pub object: ArkObject,
    pub source_links: Vec<ObjectLink>,
    pub bootstrap: Option<TaskMergeLocalBootstrapPlan>,
    pub operations: Vec<TaskMergeLocalOperation>,
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskMergeLocalBootstrapPlan {
    pub bootstrap: TaskMergeBootstrap,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskMergeLocalOperation {
    pub operation: TaskMergeOperation,
    pub expected_conflict_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskMergeLocalMutationResult {
    pub object: Option<ArkObject>,
    pub object_revision: String,
    pub link_revisions: Vec<TaskMergeLocalLinkRevision>,
    pub hard_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMergeLocalLinkRevision {
    pub link_id: String,
    pub revision: String,
}

pub fn apply_local_mutation(
    transaction: &Connection,
    mut plan: TaskMergeLocalMutationPlan,
) -> Result<TaskMergeLocalMutationResult, TaskMergeStoreError> {
    let object_id = plan.object.id.clone();
    let current_state = super::load_local_mutation_state(transaction, &object_id)?;
    if current_state != plan.expected_state {
        return Err(TaskMergeStoreError::Invalid(
            "object_conflict:stale_snapshot".into(),
        ));
    }
    let first_revision = current_state.object.is_none() && !current_state.has_bootstrap;
    let normalized_device_id = match plan.device_id.trim() {
        "" => "ark-core-rpc-local",
        device_id => device_id,
    };
    let early_revision = if first_revision {
        Some(
            bump_local_revision(
                transaction,
                LocalRevisionTarget::Object,
                &object_id,
                normalized_device_id,
            )
            .map_err(TaskMergeStoreError::Storage)?,
        )
    } else {
        None
    };

    if let Some(bootstrap_plan) = &mut plan.bootstrap {
        if bootstrap_plan.bootstrap.object.id != object_id {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_bootstrap_object_mismatch".into(),
            ));
        }
        let prepared = match check_bootstrap(transaction, &bootstrap_plan.bootstrap)? {
            TaskMergeBootstrapCheck::Replay => {
                return Err(TaskMergeStoreError::OperationIdCollision);
            }
            TaskMergeBootstrapCheck::New(prepared) => prepared,
        };
        let bootstrap = prepared.bootstrap().clone();
        let existing_object =
            crate::objects::get_object(transaction, &object_id).map_err(storage_error)?;
        if let Some(existing_object) = existing_object {
            let existing_links = current_state.source_links.clone();
            let mut expected_links = bootstrap.links.clone();
            expected_links.sort_by(|left, right| left.id.cmp(&right.id));
            if existing_object != bootstrap.object || existing_links != expected_links {
                return Err(TaskMergeStoreError::Invalid(
                    "task_merge_bootstrap_existing_object_mismatch".into(),
                ));
            }
        }
        persist_bootstrap(transaction, prepared)?;
        crate::objects::upsert_object(transaction, &bootstrap.object).map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM object_links WHERE source_object_id = ?1",
                [&object_id],
            )
            .map_err(storage_error)?;
        for link in &bootstrap.links {
            crate::objects::upsert_object_link(transaction, link).map_err(storage_error)?;
        }
    }

    let revision = match early_revision {
        Some(revision) => revision,
        None => bump_local_revision(
            transaction,
            LocalRevisionTarget::Object,
            &object_id,
            normalized_device_id,
        )
        .map_err(TaskMergeStoreError::Storage)?,
    };

    for mut local_operation in plan.operations {
        local_operation.operation.hlc = Some(revision.clone());
        let operation = canonical_operation(local_operation.operation.clone())?.0;
        let prepared = match check_operation(
            transaction,
            &operation,
            local_operation.expected_conflict_digest.as_deref(),
            TaskMergeOperationOrigin::Local,
        )? {
            TaskMergeOperationCheck::Replay(_) => continue,
            TaskMergeOperationCheck::New(prepared) => prepared,
        };
        if let Some(expected_digest) = local_operation.expected_conflict_digest {
            let current = super::read_conflict_set(transaction, &object_id, operation.unit)?;
            if current.dependencies_pending
                || current.conflict_set.as_ref().map(|set| set.digest.as_str())
                    != Some(expected_digest.as_str())
            {
                return Err(TaskMergeStoreError::Invalid(
                    "task_merge_conflict_set_changed".into(),
                ));
            }
        }
        persist_operation(transaction, prepared)?;
    }

    let outcome = materialize_task_if_ready(transaction, &object_id)?;
    let (object, hard_deleted) = match outcome {
        TaskMergeProjectionOutcome::Materialized {
            object,
            hard_deleted,
        } => (object.map(|object| *object), hard_deleted),
        TaskMergeProjectionOutcome::NoBootstrap => {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_materialized_object_missing".into(),
            ));
        }
        TaskMergeProjectionOutcome::DependenciesPending => {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_dependencies_pending".into(),
            ));
        }
    };

    let mut link_revisions = Vec::new();
    if !hard_deleted {
        for link in plan.source_links {
            crate::objects::upsert_object_link(transaction, &link).map_err(storage_error)?;
            let link_revision = bump_local_revision(
                transaction,
                LocalRevisionTarget::ObjectLink,
                &link.id,
                &plan.device_id,
            )
            .map_err(TaskMergeStoreError::Storage)?;
            link_revisions.push(TaskMergeLocalLinkRevision {
                link_id: link.id,
                revision: link_revision,
            });
        }
    }

    Ok(TaskMergeLocalMutationResult {
        object,
        object_revision: revision,
        link_revisions,
        hard_deleted,
    })
}
