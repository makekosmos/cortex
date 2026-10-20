use crate::application::legacy_mutation::{LegacyMutationSession, LegacyObjectKind};
use crate::types::SyncEntity;
use rusqlite::Transaction;

pub(crate) struct SqliteLegacyMutationSession<'tx, 'connection> {
    transaction: &'tx Transaction<'connection>,
}

impl<'tx, 'connection> SqliteLegacyMutationSession<'tx, 'connection> {
    pub(crate) fn new(transaction: &'tx Transaction<'connection>) -> Self {
        Self { transaction }
    }
}

impl LegacyMutationSession for SqliteLegacyMutationSession<'_, '_> {
    fn write_records(
        &self,
        records: &[ark_application::canonical_compatibility::LegacyRecord],
        device_id: Option<&str>,
    ) -> Result<Vec<SyncEntity>, String> {
        crate::canonical_types::facades::write_legacy_records(
            self.transaction,
            records,
            "rpc",
            device_id.map(str::to_owned),
        )
    }

    fn delete_legacy_object(
        &self,
        id: &str,
        kind: LegacyObjectKind,
        device_id: Option<&str>,
    ) -> Result<SyncEntity, String> {
        crate::canonical_types::facades::delete_legacy_object(
            self.transaction,
            id,
            kind.type_id(),
            device_id.map(str::to_owned),
        )
    }
}
