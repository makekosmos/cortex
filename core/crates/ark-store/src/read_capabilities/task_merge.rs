use super::TaskMergeReadCapabilities;
use ark_domain::task_merge::TaskMergeUnit;

impl TaskMergeReadCapabilities<'_> {
    pub fn conflict_set(
        &self,
        object_id: &str,
        unit: TaskMergeUnit,
    ) -> Result<
        ark_application::task_merge::TaskMergeConflictRead,
        crate::task_merge::TaskMergeStoreError,
    > {
        crate::task_merge::read_conflict_set(self.connection, object_id, unit)
    }
}
