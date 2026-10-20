use crate::types::SyncEntity;
use ark_application::canonical_compatibility::LegacyRecord;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub enum LegacyObjectKind {
    Task,
    Project,
}

impl LegacyObjectKind {
    pub(crate) fn type_id(self) -> &'static str {
        match self {
            Self::Task => "core.task",
            Self::Project => "com.kosmos.project",
        }
    }
}

pub trait LegacyMutationSession {
    fn write_records(
        &self,
        records: &[LegacyRecord],
        device_id: Option<&str>,
    ) -> Result<Vec<SyncEntity>, String>;
    fn delete_legacy_object(
        &self,
        id: &str,
        kind: LegacyObjectKind,
        device_id: Option<&str>,
    ) -> Result<SyncEntity, String>;
}

#[derive(Debug, Clone, Serialize)]
pub struct LegacyGraphWrite {
    pub records: Vec<LegacyRecord>,
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LegacyDelete {
    pub id: String,
    pub kind: LegacyObjectKind,
    pub device_id: Option<String>,
}

pub fn apply_legacy_graph(
    session: &dyn LegacyMutationSession,
    write: LegacyGraphWrite,
) -> Result<Vec<SyncEntity>, String> {
    session.write_records(&write.records, write.device_id.as_deref())
}

pub fn apply_legacy_delete(
    session: &dyn LegacyMutationSession,
    write: LegacyDelete,
) -> Result<SyncEntity, String> {
    session.delete_legacy_object(&write.id, write.kind, write.device_id.as_deref())
}
