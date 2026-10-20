use crate::sync_policy::PolicyIdentity;
use ark_domain::task_merge::{TaskConflictSet, TaskMergeBootstrap, TaskMergeOperation};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMergeConflictRead {
    pub conflict_set: Option<TaskConflictSet>,
    pub dependencies_pending: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMergeAppendResult {
    pub conflict_set: Option<TaskConflictSet>,
    pub dependencies_pending: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "record", rename_all = "camelCase")]
pub enum TaskMergeJournalRecord {
    Bootstrap(TaskMergeBootstrap),
    Operation {
        operation: TaskMergeOperation,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_conflict_digest: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskMergeJournalEntry {
    pub position: i64,
    pub record: TaskMergeJournalRecord,
}

impl TaskMergeJournalRecord {
    pub fn operation(&self) -> Option<(&TaskMergeOperation, Option<&str>)> {
        match self {
            Self::Bootstrap(_) => None,
            Self::Operation {
                operation,
                expected_conflict_digest,
            } => Some((operation, expected_conflict_digest.as_deref())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskMergeJournalPage {
    pub through_position: i64,
    pub entries: Vec<TaskMergeJournalEntry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskMergeBatch {
    pub batch_id: String,
    pub identity: PolicyIdentity,
    pub after_position: i64,
    pub through_position: i64,
    pub entries: Vec<TaskMergeJournalEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskMergeBatchAck {
    pub batch_id: String,
    pub through_position: i64,
    pub replayed: bool,
}

#[derive(Debug)]
pub enum TaskMergePrepareError {
    PolicyRejected,
    Failure(String),
}

#[async_trait::async_trait]
pub trait TaskMergeDeliveryPort: Send + Sync {
    async fn prepare_task_merge_batch(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        limit: usize,
    ) -> Result<Option<TaskMergeBatch>, TaskMergePrepareError>;

    async fn acknowledge_task_merge_batch(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        batch_id: &str,
        through_position: i64,
    ) -> Result<i64, String>;

    async fn apply_task_merge_batch(
        &self,
        peer_device_id: &str,
        local_device_id: &str,
        expected_identity: &PolicyIdentity,
        batch: &TaskMergeBatch,
    ) -> Result<TaskMergeBatchAck, String>;
}
