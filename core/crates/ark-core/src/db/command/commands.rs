mod core_task;
mod legacy_mutation;
mod object_mutation;
mod type_mutation;
mod usage;

pub(crate) use core_task::{AuthenticatedCoreTaskWrite, LocalCoreTaskWrite};
pub(crate) use legacy_mutation::{
    LegacyDeleteCommand, LegacyGraphWriteCommand, TrackedAppDeleteCommand, TrackedAppUpsertCommand,
};
pub(crate) use object_mutation::{
    CanonicalGameUpsertCommand, LocalBookCoverCommand, LocalObjectDeleteCommand,
    LocalObjectUpsertCommand,
};
pub(crate) use type_mutation::{
    DeleteObjectLinkCommand, DeleteObjectTypeCommand, RegisterPackageTypesCommand,
    ReplaceObjectLinkCommand, UpsertObjectLinkCommand, UpsertObjectTypeCommand,
};
pub(crate) use usage::LocalUsageWriteCommand;

fn serialized_weight(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len())
}
