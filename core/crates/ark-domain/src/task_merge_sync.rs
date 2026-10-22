use crate::task_merge::{TaskMergeBootstrap, TaskMergeOperation};

pub const MAX_TASK_MERGE_PAGE_SIZE: usize = 100;

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
pub struct TaskMergeBatch {
    pub batch_id: String,
    pub identity: crate::sync_policy::PolicyIdentity,
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
