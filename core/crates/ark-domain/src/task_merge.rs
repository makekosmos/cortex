use crate::types::{ArkObject, ObjectLink};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

mod projection;
pub use projection::{apply_unit_value, materialize_task, value_for_unit};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperationId(String);

impl OperationId {
    pub fn parse(value: impl Into<String>) -> Result<Self, MergeError> {
        let value = value.into();
        if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(MergeError::InvalidOperationId);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for OperationId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for OperationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskMergeUnit {
    Title,
    Content,
    State,
    Planning,
    Deadline,
    Reminders,
    Recurrence,
    Priority,
    Checklist,
    ObjectExistence,
}

impl TaskMergeUnit {
    pub const ALL: [Self; 10] = [
        Self::Title,
        Self::Content,
        Self::State,
        Self::Planning,
        Self::Deadline,
        Self::Reminders,
        Self::Recurrence,
        Self::Priority,
        Self::Checklist,
        Self::ObjectExistence,
    ];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum TaskMergeValue {
    Field(Value),
    FieldAbsent,
    ObjectExists,
    ObjectDeleted { deleted_at: String },
    ObjectHardDeleted { deleted_at: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMergeOperation {
    pub id: OperationId,
    pub object_id: String,
    pub unit: TaskMergeUnit,
    pub parents: Vec<OperationId>,
    /// Heads from every task unit visible when this operation was created.
    /// `parents` remains the same-unit conflict graph; this records
    /// cross-unit causality, such as a field edit observed by a later delete.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observed_heads: Vec<OperationId>,
    /// Wall-clock metadata for compatibility and display; never used to
    /// establish causal order. Omitted on legacy operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hlc: Option<String>,
    pub value: TaskMergeValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLifecycleConflict {
    pub deletion: TaskMergeOperation,
    pub concurrent_head: TaskMergeOperation,
}

/// A complete task snapshot from which every replica derives the same baseline history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMergeBootstrap {
    pub id: OperationId,
    pub object: ArkObject,
    pub links: Vec<ObjectLink>,
    pub source_hlc: String,
    pub baseline_ids: Vec<(TaskMergeUnit, OperationId)>,
}

impl TaskMergeBootstrap {
    pub fn baseline_operations(&self) -> Result<Vec<TaskMergeOperation>, MergeError> {
        let source_clock = crate::hlc::HLC::from_string(&self.source_hlc);
        if self.object.type_id != "core.task"
            || self.object.type_version != "0.1.0"
            || self.object.id.trim().is_empty()
            || source_clock.to_string() != self.source_hlc
            || source_clock.device_id.trim().is_empty()
            || chrono::DateTime::parse_from_rfc3339(&source_clock.wall_time).is_err()
        {
            return Err(MergeError::InvalidBootstrap);
        }
        if self.links.iter().any(|link| {
            link.source_object_id != self.object.id
                || link.id.trim().is_empty()
                || link.target_object_id.trim().is_empty()
                || link.link_type.trim().is_empty()
                || chrono::DateTime::parse_from_rfc3339(&link.created_at).is_err()
        }) || self
            .links
            .iter()
            .map(|link| link.id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != self.links.len()
            || self.baseline_ids.len() != TaskMergeUnit::ALL.len()
        {
            return Err(MergeError::InvalidBootstrap);
        }

        let ids = self
            .baseline_ids
            .iter()
            .map(|(_, id)| id)
            .collect::<BTreeSet<_>>();
        if ids.len() != self.baseline_ids.len() || ids.contains(&&self.id) {
            return Err(MergeError::InvalidBootstrap);
        }

        TaskMergeUnit::ALL
            .iter()
            .zip(&self.baseline_ids)
            .map(|(expected_unit, (unit, id))| {
                if unit != expected_unit {
                    return Err(MergeError::InvalidBootstrap);
                }
                Ok(TaskMergeOperation {
                    id: id.clone(),
                    object_id: self.object.id.clone(),
                    unit: *unit,
                    parents: Vec::new(),
                    observed_heads: Vec::new(),
                    hlc: Some(self.source_hlc.clone()),
                    value: value_for_unit(&self.object, *unit)?,
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskConflictSet {
    pub object_id: String,
    pub unit: TaskMergeUnit,
    pub common_ancestors: Vec<TaskMergeOperation>,
    pub variants: Vec<TaskMergeOperation>,
    pub digest: String,
}

impl TaskConflictSet {
    pub fn has_conflict(&self) -> bool {
        self.variants
            .first()
            .is_some_and(|first| self.variants.iter().any(|other| other.value != first.value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MergeError {
    #[error("merge operation id must contain 32 hexadecimal characters")]
    InvalidOperationId,
    #[error("merge history contains duplicate operation ids with different data")]
    OperationIdCollision,
    #[error("merge history contains an operation for another object or field")]
    MixedHistory,
    #[error("merge history refers to a missing parent")]
    MissingParent,
    #[error("merge history contains a causal cycle")]
    CausalCycle,
    #[error("merge operation lists the same parent more than once")]
    DuplicateParent,
    #[error("merge operation lists the same observed head more than once")]
    DuplicateObservedHead,
    #[error("merge operation has an empty object id")]
    EmptyObjectId,
    #[error("merge operation has an invalid HLC")]
    InvalidHlc,
    #[error("merge history is empty")]
    EmptyHistory,
    #[error("merge unit and value kind do not match")]
    UnitValueMismatch,
    #[error("merge conflict set could not be encoded")]
    DigestEncoding,
    #[error("task merge value cannot be applied to the canonical task")]
    InvalidTaskPayload,
    #[error("task merge projection contains more than one conflict set for a unit")]
    DuplicateMaterializedUnit,
    #[error("task merge bootstrap is incomplete or inconsistent")]
    InvalidBootstrap,
}

pub fn validate_operation(operation: &TaskMergeOperation) -> Result<(), MergeError> {
    if operation.object_id.trim().is_empty() {
        return Err(MergeError::EmptyObjectId);
    }
    let unique_parents = operation.parents.iter().collect::<BTreeSet<_>>();
    if unique_parents.len() != operation.parents.len() {
        return Err(MergeError::DuplicateParent);
    }
    let unique_observed_heads = operation.observed_heads.iter().collect::<BTreeSet<_>>();
    if unique_observed_heads.len() != operation.observed_heads.len() {
        return Err(MergeError::DuplicateObservedHead);
    }
    if operation.parents.contains(&operation.id) {
        return Err(MergeError::CausalCycle);
    }
    if operation.observed_heads.contains(&operation.id) {
        return Err(MergeError::CausalCycle);
    }
    if operation.hlc.as_deref().is_some_and(|hlc| !valid_hlc(hlc)) {
        return Err(MergeError::InvalidHlc);
    }
    let value_matches_unit = matches!(
        (operation.unit, &operation.value),
        (TaskMergeUnit::ObjectExistence, TaskMergeValue::ObjectExists)
            | (
                TaskMergeUnit::ObjectExistence,
                TaskMergeValue::ObjectDeleted { .. }
            )
            | (
                TaskMergeUnit::ObjectExistence,
                TaskMergeValue::ObjectHardDeleted { .. }
            )
            | (
                TaskMergeUnit::Title
                    | TaskMergeUnit::Content
                    | TaskMergeUnit::State
                    | TaskMergeUnit::Planning
                    | TaskMergeUnit::Deadline
                    | TaskMergeUnit::Reminders
                    | TaskMergeUnit::Recurrence
                    | TaskMergeUnit::Priority
                    | TaskMergeUnit::Checklist,
                TaskMergeValue::Field(_) | TaskMergeValue::FieldAbsent
            )
    );
    if !value_matches_unit {
        return Err(MergeError::UnitValueMismatch);
    }
    Ok(())
}

fn valid_hlc(value: &str) -> bool {
    let parsed = crate::hlc::HLC::from_string(value);
    chrono::DateTime::parse_from_rfc3339(&parsed.wall_time).is_ok()
        && !parsed.device_id.trim().is_empty()
        && parsed.to_string() == value
}

pub fn conflict_set(history: &[TaskMergeOperation]) -> Result<TaskConflictSet, MergeError> {
    let first = history.first().ok_or(MergeError::EmptyHistory)?;
    let mut operations = BTreeMap::new();
    for operation in history {
        if operation.object_id != first.object_id || operation.unit != first.unit {
            return Err(MergeError::MixedHistory);
        }
        validate_operation(operation)?;
        let mut canonical_operation = operation.clone();
        canonical_operation.parents.sort();
        canonical_operation.observed_heads.sort();
        if let Some(previous) =
            operations.insert(canonical_operation.id.clone(), canonical_operation.clone())
        {
            if previous != canonical_operation {
                return Err(MergeError::OperationIdCollision);
            }
        }
    }

    let mut children = BTreeSet::new();
    for operation in operations.values() {
        for parent in &operation.parents {
            operations.get(parent).ok_or(MergeError::MissingParent)?;
            children.insert(parent.clone());
        }
    }
    let ancestor_map = operations
        .keys()
        .map(|id| Ok((id.clone(), ancestors(id, &operations)?)))
        .collect::<Result<BTreeMap<_, _>, MergeError>>()?;

    let variants = operations
        .iter()
        .filter(|(id, _)| !children.contains(*id))
        .map(|(_, operation)| operation.clone())
        .collect::<Vec<_>>();
    let ancestor_sets = variants
        .iter()
        .map(|variant| ancestor_map[&variant.id].clone())
        .collect::<Vec<_>>();
    let common = ancestor_sets.iter().skip(1).fold(
        ancestor_sets.first().cloned().unwrap_or_default(),
        |common, next| common.intersection(next).cloned().collect(),
    );
    let maximal_common = common
        .iter()
        .filter(|candidate| {
            !common
                .iter()
                .any(|other| candidate != &other && ancestor_map[other].contains(*candidate))
        })
        .filter_map(|id| operations.get(id).cloned())
        .collect::<Vec<_>>();

    let mut result = TaskConflictSet {
        object_id: first.object_id.clone(),
        unit: first.unit,
        common_ancestors: maximal_common,
        variants,
        digest: String::new(),
    };
    result.digest = conflict_digest(&result)?;
    Ok(result)
}

/// Finds field heads that are concurrent with a currently visible deletion.
/// The function preserves the operation graph and never chooses whether to
/// restore the task.
pub fn lifecycle_conflicts(
    history: &[TaskMergeOperation],
) -> Result<Vec<TaskLifecycleConflict>, MergeError> {
    let first = history.first().ok_or(MergeError::EmptyHistory)?;
    let mut operations = BTreeMap::new();
    let mut by_unit = BTreeMap::<TaskMergeUnit, Vec<TaskMergeOperation>>::new();
    for operation in history {
        if operation.object_id != first.object_id {
            return Err(MergeError::MixedHistory);
        }
        validate_operation(operation)?;
        let mut canonical = operation.clone();
        canonical.parents.sort();
        canonical.observed_heads.sort();
        if let Some(previous) = operations.insert(canonical.id.clone(), canonical.clone()) {
            if previous != canonical {
                return Err(MergeError::OperationIdCollision);
            }
        }
        by_unit.entry(operation.unit).or_default().push(canonical);
    }

    let ancestors = operations
        .keys()
        .map(|id| Ok((id.clone(), causal_ancestors(id, &operations)?)))
        .collect::<Result<BTreeMap<_, _>, MergeError>>()?;
    let Some(existence) = by_unit.get(&TaskMergeUnit::ObjectExistence) else {
        return Ok(Vec::new());
    };
    let lifecycle = conflict_set(existence)?;
    let mut conflicts = Vec::new();

    for deletion in lifecycle.variants.into_iter().filter(|operation| {
        matches!(
            operation.value,
            TaskMergeValue::ObjectDeleted { .. } | TaskMergeValue::ObjectHardDeleted { .. }
        )
    }) {
        for unit in TaskMergeUnit::ALL {
            if unit == TaskMergeUnit::ObjectExistence {
                continue;
            }
            let Some(unit_history) = by_unit.get(&unit) else {
                continue;
            };
            for head in conflict_set(unit_history)?.variants {
                if !ancestors[&deletion.id].contains(&head.id)
                    && !ancestors[&head.id].contains(&deletion.id)
                {
                    conflicts.push(TaskLifecycleConflict {
                        deletion: deletion.clone(),
                        concurrent_head: head,
                    });
                }
            }
        }
    }

    conflicts.sort_by(|left, right| {
        (&left.deletion.id, &left.concurrent_head.id)
            .cmp(&(&right.deletion.id, &right.concurrent_head.id))
    });
    Ok(conflicts)
}

fn causal_ancestors(
    id: &OperationId,
    operations: &BTreeMap<OperationId, TaskMergeOperation>,
) -> Result<BTreeSet<OperationId>, MergeError> {
    let operation = operations.get(id).ok_or(MergeError::MissingParent)?;
    let mut pending = operation
        .parents
        .iter()
        .chain(&operation.observed_heads)
        .cloned()
        .collect::<Vec<_>>();
    let mut found = BTreeSet::new();
    while let Some(parent) = pending.pop() {
        if !found.insert(parent.clone()) {
            continue;
        }
        let parent_operation = operations.get(&parent).ok_or(MergeError::MissingParent)?;
        if parent_operation.id == *id {
            return Err(MergeError::CausalCycle);
        }
        pending.extend(parent_operation.parents.iter().cloned());
        pending.extend(parent_operation.observed_heads.iter().cloned());
    }
    Ok(found)
}

fn ancestors(
    id: &OperationId,
    operations: &BTreeMap<OperationId, TaskMergeOperation>,
) -> Result<BTreeSet<OperationId>, MergeError> {
    let mut found = BTreeSet::new();
    let mut pending = operations
        .get(id)
        .ok_or(MergeError::MissingParent)?
        .parents
        .clone();
    while let Some(parent) = pending.pop() {
        if !found.insert(parent.clone()) {
            continue;
        }
        let operation = operations.get(&parent).ok_or(MergeError::MissingParent)?;
        if operation.id == *id {
            return Err(MergeError::CausalCycle);
        }
        pending.extend(operation.parents.iter().cloned());
    }
    Ok(found)
}

fn conflict_digest(conflicts: &TaskConflictSet) -> Result<String, MergeError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DigestInput<'a> {
        object_id: &'a str,
        unit: TaskMergeUnit,
        common_ancestors: &'a [TaskMergeOperation],
        variants: &'a [TaskMergeOperation],
    }
    let bytes = serde_json::to_vec(&DigestInput {
        object_id: &conflicts.object_id,
        unit: conflicts.unit,
        common_ancestors: &conflicts.common_ancestors,
        variants: &conflicts.variants,
    })
    .map_err(|_| MergeError::DigestEncoding)?;
    let mut hasher = Sha256::new();
    hasher.update(b"ark-task-conflict-set-v1\0");
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

#[cfg(test)]
#[path = "task_merge/tests.rs"]
mod tests;
