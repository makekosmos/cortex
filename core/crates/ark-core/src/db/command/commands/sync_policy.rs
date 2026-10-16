use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError, DbQuery};
use crate::data_platform::{AcceptedPeerSyncPolicy, SyncProfile, SyncProfileRule};
use rusqlite::{Connection, Transaction};

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

impl sealed::Sealed for SaveLocalSyncProfileCommand {}

impl DbCommand for SaveLocalSyncProfileCommand {
    type Output = SyncProfile;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
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

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        crate::data_platform::upsert_sync_profile(
            transaction,
            &self.device_id,
            &self.profile_id,
            &self.name,
            self.active,
            self.expected_revision,
            &self.rules,
        )
        .map_err(map_profile_error)
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

impl DbQuery for LoadActiveLocalSyncProfileQuery {
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
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        crate::data_platform::load_active_sync_profile(connection, &self.device_id)
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
    rules: Vec<SyncProfileRule>,
}

impl AcceptPeerSyncPolicyCommand {
    pub(crate) fn new(
        peer_device_id: String,
        revision: i64,
        digest: String,
        rules: Vec<SyncProfileRule>,
    ) -> Self {
        Self {
            peer_device_id,
            revision,
            digest,
            rules,
        }
    }
}

impl sealed::Sealed for AcceptPeerSyncPolicyCommand {}

impl DbCommand for AcceptPeerSyncPolicyCommand {
    type Output = AcceptedPeerSyncPolicy;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
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

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        crate::data_platform::accept_peer_sync_policy(
            transaction,
            &self.peer_device_id,
            self.revision,
            &self.digest,
            &self.rules,
        )
        .map_err(map_profile_error)
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

impl DbQuery for LoadAcceptedPeerSyncPolicyQuery {
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
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        crate::data_platform::load_accepted_peer_sync_policy(connection, &self.peer_device_id)
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

impl DbQuery for LoadSyncProfileForSyncQuery {
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
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        crate::data_platform::load_active_sync_profile(connection, &self.local_device_id)
            .map_err(map_profile_error)
    }
}
