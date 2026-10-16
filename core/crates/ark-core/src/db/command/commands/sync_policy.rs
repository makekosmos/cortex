use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError, DbQuery};
use crate::data_platform::{SyncProfile, SyncProfileRule};
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
