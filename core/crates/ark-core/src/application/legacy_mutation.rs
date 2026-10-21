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
