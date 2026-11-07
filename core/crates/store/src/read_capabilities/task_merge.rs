use super::TaskMergeReadCapabilities;
use domain::task_merge::{TaskMergeConflictRead, TaskMergeUnit};

impl TaskMergeReadCapabilities<'_> {
    pub fn conflict_set(
        &self,
        object_id: &str,
        unit: TaskMergeUnit,
    ) -> Result<TaskMergeConflictRead, crate::task_merge::TaskMergeStoreError> {
        crate::task_merge::read_conflict_set(self.connection, object_id, unit)
    }
}
