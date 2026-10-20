use crate::application::legacy_mutation::LegacyDelete;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LegacyDeleteCommand {
    pub(crate) write: LegacyDelete,
}

impl LegacyDeleteCommand {
    pub(crate) fn new(write: LegacyDelete) -> Self {
        Self { write }
    }
}

impl LegacyDeleteCommand {
    pub(crate) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }
}
