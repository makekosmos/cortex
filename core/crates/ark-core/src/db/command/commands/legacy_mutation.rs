mod legacy_delete;
mod legacy_graph;
mod tracked_app;

pub(crate) use legacy_delete::LegacyDeleteCommand;
pub(crate) use legacy_graph::LegacyGraphWriteCommand;
pub(crate) use tracked_app::{TrackedAppDeleteCommand, TrackedAppUpsertCommand};

fn serialized_weight(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len())
}
