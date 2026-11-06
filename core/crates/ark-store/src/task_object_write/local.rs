use crate::object_write::StoredObjectReceipt;
use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ArkObjectWrite};

#[derive(Clone, Debug, serde::Serialize)]
pub struct LocalCoreTaskWrite {
    object: ArkObjectWrite,
    expected_snapshot: Option<ObjectWriteSnapshot>,
    device_id: Option<String>,
}

impl LocalCoreTaskWrite {
    pub fn new(
        object: ArkObjectWrite,
        expected_snapshot: Option<ObjectWriteSnapshot>,
        device_id: Option<String>,
    ) -> Self {
        Self {
            object,
            expected_snapshot,
            device_id,
        }
    }

    pub fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }

    pub fn object_type_id(&self) -> &str {
        &self.object.type_id
    }

    pub fn into_parts(self) -> (ArkObjectWrite, Option<ObjectWriteSnapshot>, Option<String>) {
        (self.object, self.expected_snapshot, self.device_id)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LocalCoreTaskWriteCommit {
    pub object: ArkObject,
    pub receipt: StoredObjectReceipt,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TaskWriteOperationResult {
    pub value: LocalCoreTaskWriteCommit,
    pub replayed: bool,
}
