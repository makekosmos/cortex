use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::canonical_types::pending::ReplayNotice;
use crate::db::{set_sync_kv, SqliteStorageBackend};
use crate::types::SyncEntity;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApplySyncEntityCommand {
    entity: SyncEntity,
}

impl ApplySyncEntityCommand {
    pub(crate) fn new(entity: SyncEntity) -> Self {
        Self { entity }
    }
}

impl sealed::Sealed for ApplySyncEntityCommand {}

impl DbCommand for ApplySyncEntityCommand {
    type Output = Vec<ReplayNotice>;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        SqliteStorageBackend::apply_entity_in_transaction(transaction, &self.entity)
            .map_err(DbError::from_application_error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApplyAuthenticatedSyncEntityCommand {
    entity: SyncEntity,
}

impl ApplyAuthenticatedSyncEntityCommand {
    pub(crate) fn new(entity: SyncEntity) -> Self {
        Self { entity }
    }
}

impl sealed::Sealed for ApplyAuthenticatedSyncEntityCommand {}

impl DbCommand for ApplyAuthenticatedSyncEntityCommand {
    type Output = Vec<ReplayNotice>;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        let peer_id = context.authenticated_peer_id()?;
        SqliteStorageBackend::apply_authenticated_entity_in_transaction(
            transaction,
            &self.entity,
            peer_id,
        )
        .map_err(DbError::from_application_error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetSyncKeyValueCommand {
    key: String,
    value: String,
}

impl SetSyncKeyValueCommand {
    pub(crate) fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl sealed::Sealed for SetSyncKeyValueCommand {}

impl DbCommand for SetSyncKeyValueCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        set_sync_kv(transaction, &self.key, &self.value).map_err(DbError::Storage)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetLocalSyncKeyValueCommand {
    key: String,
    value: String,
}

impl SetLocalSyncKeyValueCommand {
    pub(crate) fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl sealed::Sealed for SetLocalSyncKeyValueCommand {}

impl DbCommand for SetLocalSyncKeyValueCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        set_sync_kv(transaction, &self.key, &self.value).map_err(DbError::Storage)
    }
}
