use crate::application::legacy_mutation::LegacyGraphWrite;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LegacyGraphWriteCommand {
    pub(crate) write: LegacyGraphWrite,
}

impl LegacyGraphWriteCommand {
    pub(crate) fn new(write: LegacyGraphWrite) -> Self {
        Self { write }
    }
}

impl LegacyGraphWriteCommand {
    pub(crate) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }
}
