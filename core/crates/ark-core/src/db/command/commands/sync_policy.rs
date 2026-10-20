use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};
use ark_application::sync_policy::{AcceptedPeerSyncPolicy, SyncProfile, SyncProfileRule};

pub(crate) struct SaveLocalSyncProfileCommand {
    device_id: String,
    profile_id: String,
    name: String,
    active: bool,
    expected_revision: Option<i64>,
    rules: Vec<SyncProfileRule>,
}

impl SaveLocalSyncProfileCommand {
    pub(crate) fn new(
        device_id: String,
        profile_id: String,
        name: String,
        active: bool,
        expected_revision: Option<i64>,
        rules: Vec<SyncProfileRule>,
    ) -> Self {
        Self {
            device_id,
            profile_id,
            name,
            active,
            expected_revision,
            rules,
        }
    }
}

impl SaveLocalSyncProfileCommand {
    pub(super) fn weight(&self) -> usize {
        self.device_id.len()
            + self.profile_id.len()
            + self.name.len()
            + self
                .rules
                .iter()
                .map(|rule| {
                    rule.resource_kind.len()
                        + rule.resource_id.len()
                        + rule.filter.canonical_json().len()
                })
                .sum::<usize>()
            + 1
    }
}

pub(crate) enum SyncPolicyWrite {
    SaveLocal(SaveLocalSyncProfileCommand),
    AcceptPeer(AcceptPeerSyncPolicyCommand),
}

pub(crate) enum SyncPolicyWriteOutput {
    LocalProfile(SyncProfile),
    AcceptedPeer(AcceptedPeerSyncPolicy),
}

impl SyncPolicyWrite {
    pub(in crate::db::command) fn class(&self) -> CommandClass {
        match self {
            Self::SaveLocal(_) => CommandClass::Interactive,
            Self::AcceptPeer(_) => CommandClass::SyncLive,
        }
    }

    pub(in crate::db::command) fn weight(&self) -> usize {
        match self {
            Self::SaveLocal(command) => command.weight(),
            Self::AcceptPeer(command) => command.weight(),
        }
    }

    pub(in crate::db::command) fn execute(
        self,
        context: &CommandContext,
        writer: &mut ark_store::writer::SqliteWriter,
    ) -> Result<SyncPolicyWriteOutput, DbError> {
        match self {
            Self::SaveLocal(command) => {
                context.require_engine()?;
                writer
                    .save_local_sync_profile(
                        &command.device_id,
                        &command.profile_id,
                        &command.name,
                        command.active,
                        command.expected_revision,
                        &command.rules,
                    )
                    .map(SyncPolicyWriteOutput::LocalProfile)
                    .map_err(|failure| map_store_profile_failure(failure, map_profile_write_error))
            }
            Self::AcceptPeer(command) => {
                context.require_sync_peer_matches(&command.peer_device_id)?;
                writer
                    .accept_peer_sync_policy(
                        &command.peer_device_id,
                        command.revision,
                        &command.digest,
                        command.active,
                        &command.rules,
                    )
                    .map(SyncPolicyWriteOutput::AcceptedPeer)
                    .map_err(|failure| map_store_profile_failure(failure, map_profile_error))
            }
        }
    }

    #[cfg(debug_assertions)]
    pub(in crate::db::command) fn execute_on_connection(
        self,
        context: &CommandContext,
        connection: &mut rusqlite::Connection,
    ) -> Result<SyncPolicyWriteOutput, DbError> {
        use ark_store::writer::TransactionFailure;

        match self {
            Self::SaveLocal(command) => {
                context.require_engine()?;
                ark_store::writer::SqliteWriter::save_local_sync_profile_on_connection(
                    connection,
                    &command.device_id,
                    &command.profile_id,
                    &command.name,
                    command.active,
                    command.expected_revision,
                    &command.rules,
                )
                .map(SyncPolicyWriteOutput::LocalProfile)
                .map_err(|failure| match failure {
                    TransactionFailure::Begin(error) => DbError::from_sqlite(error),
                    TransactionFailure::Operation(error) => map_profile_write_error(error),
                    TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
                })
            }
            Self::AcceptPeer(command) => {
                context.require_sync_peer_matches(&command.peer_device_id)?;
                ark_store::writer::SqliteWriter::accept_peer_sync_policy_on_connection(
                    connection,
                    &command.peer_device_id,
                    command.revision,
                    &command.digest,
                    command.active,
                    &command.rules,
                )
                .map(SyncPolicyWriteOutput::AcceptedPeer)
                .map_err(|failure| match failure {
                    TransactionFailure::Begin(error) => DbError::from_sqlite(error),
                    TransactionFailure::Operation(error) => map_profile_error(error),
                    TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
                })
            }
        }
    }
}

fn map_store_profile_failure<E>(
    failure: ark_store::writer::StoreTransactionFailure<E>,
    map_operation: impl FnOnce(E) -> DbError,
) -> DbError {
    use ark_store::writer::{StoreErrorKind, StoreTransactionFailure};
    match failure {
        StoreTransactionFailure::Begin(error) => match error.kind {
            StoreErrorKind::DiskFull | StoreErrorKind::Io | StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        StoreTransactionFailure::Operation(error) => map_operation(error),
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}

fn map_profile_write_error(error: ark_store::data_platform::SyncProfileWriteError) -> DbError {
    use ark_store::data_platform::SyncProfileWriteError;
    match error {
        SyncProfileWriteError::Operation(message) => map_profile_error(message),
        SyncProfileWriteError::SavepointRelease(cleanup) => {
            DbError::Storage(format!("sync profile savepoint release failed: {cleanup}"))
        }
        SyncProfileWriteError::SavepointCleanup { operation, cleanup } => map_profile_error(
            format!("{operation}; sync profile savepoint rollback failed: {cleanup}"),
        ),
    }
}

pub(crate) struct LoadActiveLocalSyncProfileQuery {
    device_id: String,
}

impl LoadActiveLocalSyncProfileQuery {
    pub(crate) fn new(device_id: String) -> Self {
        Self { device_id }
    }
}

impl sealed::Sealed for LoadActiveLocalSyncProfileQuery {}

impl StoreReadQuery for LoadActiveLocalSyncProfileQuery {
    type Output = Option<SyncProfile>;

    fn class(&self) -> CommandClass {
        CommandClass::Interactive
    }

    fn weight(&self) -> usize {
        self.device_id.len().max(1)
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        capabilities
            .sync()
            .active_local_sync_profile(&self.device_id)
            .map_err(map_profile_error)
    }
}

fn map_profile_error(error: String) -> DbError {
    match error.as_str() {
        "PROFILE_OWNER_DENIED" => DbError::Permission(error),
        "PROFILE_REVISION_STALE" => DbError::Conflict(error),
        message if message.starts_with("invalid") || message.starts_with("too many") => {
            DbError::Invalid(error)
        }
        _ => DbError::Storage(error),
    }
}

pub(crate) struct AcceptPeerSyncPolicyCommand {
    peer_device_id: String,
    revision: i64,
    digest: String,
    active: bool,
    rules: Vec<SyncProfileRule>,
}

impl AcceptPeerSyncPolicyCommand {
    pub(crate) fn new(
        peer_device_id: String,
        revision: i64,
        digest: String,
        active: bool,
        rules: Vec<SyncProfileRule>,
    ) -> Self {
        Self {
            peer_device_id,
            revision,
            digest,
            active,
            rules,
        }
    }
}

impl AcceptPeerSyncPolicyCommand {
    pub(super) fn weight(&self) -> usize {
        self.peer_device_id.len()
            + self.digest.len()
            + self
                .rules
                .iter()
                .map(|rule| {
                    rule.resource_kind.len()
                        + rule.resource_id.len()
                        + rule.filter.canonical_json().len()
                })
                .sum::<usize>()
    }
}

pub(crate) struct LoadAcceptedPeerSyncPolicyQuery {
    peer_device_id: String,
}

impl LoadAcceptedPeerSyncPolicyQuery {
    pub(crate) fn new(peer_device_id: String) -> Self {
        Self { peer_device_id }
    }
}

impl sealed::Sealed for LoadAcceptedPeerSyncPolicyQuery {}

impl StoreReadQuery for LoadAcceptedPeerSyncPolicyQuery {
    type Output = Option<AcceptedPeerSyncPolicy>;

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        self.peer_device_id.len().max(1)
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        capabilities
            .sync()
            .accepted_peer_policy(&self.peer_device_id)
            .map_err(map_profile_error)
    }
}

pub(crate) struct LoadSyncProfileForSyncQuery {
    local_device_id: String,
}

impl LoadSyncProfileForSyncQuery {
    pub(crate) fn new(local_device_id: String) -> Self {
        Self { local_device_id }
    }
}

impl sealed::Sealed for LoadSyncProfileForSyncQuery {}

impl StoreReadQuery for LoadSyncProfileForSyncQuery {
    type Output = Option<SyncProfile>;

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        self.local_device_id.len().max(1)
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        capabilities
            .sync()
            .sync_profile(&self.local_device_id)
            .map_err(map_profile_error)
    }
}
