mod legacy_delete;
mod legacy_graph;

pub(crate) use legacy_delete::LegacyDeleteCommand;
pub(crate) use legacy_graph::LegacyGraphWriteCommand;

fn serialized_weight(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len())
}
