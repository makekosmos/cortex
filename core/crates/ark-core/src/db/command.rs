use rusqlite::{Connection, Transaction};
use std::sync::Arc;
#[cfg(debug_assertions)]
use std::sync::Mutex;

mod commands;
mod executor;
mod handle;
pub(crate) use handle::{DbHandle, DbOwner};

pub(crate) use commands::{
    AcquireIntegrationRefreshLeaseCommand, ApplyAuthenticatedSyncEntityCommand,
    ApplySignedIntegrationFrameCommand, ApplySyncEntityCommand, AuthenticatedCoreTaskWrite,
    AuthorizedTransportKeyQuery, CanonicalGameUpsertCommand, ClearAllCommand,
    CompactUsageSyncLogCommand, DeleteObjectLinkCommand, DeleteObjectTypeCommand,
    DeleteTrashedCommand, EnsureUsageSequenceMigratedCommand, GetSyncKeyValueQuery,
    IdentityErrorStyle, IntegrationReadQuery, LegacyDeleteCommand, LegacyGraphWriteCommand,
    LoadEntitiesQuery, LoadOrGenerateIrohSecretKeyCommand, LocalBookCoverCommand,
    LocalCoreTaskWrite, LocalObjectDeleteCommand, LocalObjectUpsertCommand, LocalUsageWriteCommand,
    ObjectReadQuery, PersistIntegrationGrantCommand, PersistNodeAuthorizationCommand,
    PublishIntegrationCredentialEnvelopeCommand, RegisterPackageTypesCommand,
    ReplaceObjectLinkCommand, RestoreSnapshotCommand, SetLocalSyncKeyValueCommand,
    SetSyncKeyValueCommand, SystemReadQuery, TrackedAppDeleteCommand, TrackedAppUpsertCommand,
    TypeReadQuery, UpsertExternalRefCommand, UpsertObjectLinkCommand, UpsertObjectTypeCommand,
    UsageCompleteThroughQuery, UsageReadQuery, ValidateOutboundSignedFrameQuery,
};

// A full transport frame is admitted at 16 MiB. Leave bounded room for the
// sealed query/command metadata that wraps the same frame for DbHandle.
const MAX_COMMAND_BYTES: usize = 16 * 1024 * 1024 + 4 * 1024;
const DB_ADMISSION_LIMITS: executor::AdmissionLimits = executor::AdmissionLimits {
    max_commands: 128,
    max_bytes: 64 * 1024 * 1024,
    max_command_bytes: MAX_COMMAND_BYTES,
    max_commands_per_source: 24,
    max_bytes_per_source: 20 * 1024 * 1024,
    max_sources: 64,
    reserved_interactive_commands: 16,
    reserved_interactive_bytes: 16 * 1024 * 1024,
    reserved_sync_live_commands: 16,
    reserved_sync_live_bytes: 8 * 1024 * 1024,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CommandClass {
    Interactive,
    SyncLive,
    Backfill,
    Maintenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommandSource {
    Engine,
    System,
    ApplicationPackage(String),
    DeviceId(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandContext {
    class: CommandClass,
    source: CommandSource,
}

impl CommandContext {
    /// Build a context from a principal already authenticated by the host or
    /// transport boundary. This API is crate-private and never reads JSON.
    pub(crate) fn from_trusted_principal(class: CommandClass, source: CommandSource) -> Self {
        Self { class, source }
    }

    pub(crate) fn engine() -> Self {
        Self::from_trusted_principal(CommandClass::Interactive, CommandSource::Engine)
    }

    pub(crate) fn engine_maintenance() -> Self {
        Self::from_trusted_principal(CommandClass::Maintenance, CommandSource::Engine)
    }

    pub(crate) fn authenticated_peer(peer_id: String) -> Self {
        Self::from_trusted_principal(CommandClass::SyncLive, CommandSource::DeviceId(peer_id))
    }

    pub(crate) fn sync_system(class: CommandClass) -> Self {
        Self::from_trusted_principal(class, CommandSource::System)
    }

    pub(crate) fn sync_peer(peer_id: String, class: CommandClass) -> Self {
        Self::from_trusted_principal(class, CommandSource::DeviceId(peer_id))
    }

    fn authenticated_peer_id(&self) -> Result<&str, DbError> {
        match &self.source {
            CommandSource::DeviceId(peer_id) => Ok(peer_id),
            CommandSource::Engine | CommandSource::System => Err(DbError::Permission(
                "authenticated sync principal required".into(),
            )),
            CommandSource::ApplicationPackage(_) => Err(DbError::Permission(
                "authenticated sync principal required".into(),
            )),
        }
    }

    pub(crate) fn require_system_sync(&self) -> Result<(), DbError> {
        if self.class != CommandClass::SyncLive && self.class != CommandClass::Backfill {
            return Err(DbError::Invalid("sync query class required".into()));
        }
        match &self.source {
            CommandSource::System => Ok(()),
            _ => Err(DbError::Permission("system sync principal required".into())),
        }
    }

    pub(crate) fn require_sync_peer_or_system(&self) -> Result<(), DbError> {
        if self.class != CommandClass::SyncLive && self.class != CommandClass::Backfill {
            return Err(DbError::Invalid("sync query class required".into()));
        }
        match &self.source {
            CommandSource::System | CommandSource::DeviceId(_) => Ok(()),
            CommandSource::ApplicationPackage(_) => {
                Err(DbError::Permission("sync principal required".into()))
            }
            CommandSource::Engine => Err(DbError::Permission("sync principal required".into())),
        }
    }

    fn require_engine(&self) -> Result<(), DbError> {
        match &self.source {
            CommandSource::Engine => Ok(()),
            CommandSource::DeviceId(_)
            | CommandSource::ApplicationPackage(_)
            | CommandSource::System => Err(DbError::Permission("engine principal required".into())),
        }
    }
}

mod sealed {
    pub trait Sealed {}
}

pub(crate) trait DbCommand: sealed::Sealed + Send + 'static {
    type Output: Send + 'static;

    const CLASS: CommandClass;

    fn weight(&self) -> usize;

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError>;
}

pub(crate) trait DbMaintenanceCommand: sealed::Sealed + Send + 'static {
    type Output: Send + 'static;

    fn weight(&self) -> usize;

    fn execute(self, connection: &mut Connection) -> Result<Self::Output, DbError>;
}

/// Sealed read-only application surface. Query implementations receive a
/// connection without opening an IMMEDIATE transaction and inspect their
/// typed principal through `CommandContext`.
pub(crate) trait DbQuery: sealed::Sealed + Send + 'static {
    type Output: Send + 'static;

    fn class(&self) -> CommandClass;

    fn weight(&self) -> usize;

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError>;
}

#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum DbError {
    #[error("database command admission is saturated")]
    Busy,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Permission(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    Storage(String),
    #[error("database command exceeds admission size limits")]
    TooLarge,
    #[error("database command worker is unavailable")]
    Unavailable,
    #[error("database actor is recovering")]
    RecoveryRequired,
    #[error("database command outcome is unknown")]
    OutcomeUnknown,
    #[error("database command was canceled before it started")]
    CancelledBeforeStart,
}

impl DbError {
    pub(crate) fn from_application_error(message: String) -> Self {
        if message.starts_with("object_conflict:") || message == "object_revision_payload_mismatch"
        {
            Self::Conflict(message)
        } else if message == "sync_peer_removed"
            || message == "authenticated sync principal required"
        {
            Self::Permission(message)
        } else if message.starts_with("object_write_slice_unsupported")
            || message.starts_with("object_write_slice_requires")
            || message.starts_with("unknown_type_or_version")
        {
            Self::Unsupported(message)
        } else if message.starts_with("invalid") || message == "object_write_origin_mismatch" {
            Self::Invalid(message)
        } else {
            Self::Storage(message)
        }
    }
}

#[cfg(test)]
#[path = "command/tests/mod.rs"]
mod tests;
