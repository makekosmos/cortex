use super::super::{sealed, CommandClass, CommandContext, DbError, DbQuery};
use rusqlite::OptionalExtension;

fn serialize<T: serde::Serialize>(value: Result<T, String>) -> Result<serde_json::Value, DbError> {
    let value = value.map_err(DbError::Storage)?;
    serde_json::to_value(value).map_err(|error| DbError::Storage(error.to_string()))
}

#[derive(serde::Serialize)]
pub(crate) enum ObjectReadQuery {
    LoadAll,
    List,
    ListSummaries,
    ListByType(String),
    ListSummariesByType(String),
    EstimateDataVolume(Vec<crate::service::DataVolumeScope>),
    ListRunningTime(Option<String>),
    GetByIds(Vec<String>),
    Search(String),
    Get(String),
    WriteSnapshot(String),
    CanonicalGames(String),
    CanonicalGame(String, String),
    CanonicalAssetSources(Vec<String>),
}

impl sealed::Sealed for ObjectReadQuery {}

impl DbQuery for ObjectReadQuery {
    type Output = serde_json::Value;

    fn class(&self) -> CommandClass {
        CommandClass::Interactive
    }

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &rusqlite::Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        match self {
            Self::LoadAll => serialize(crate::db::load_all(connection)),
            Self::List => serialize(crate::db::list_objects(connection)),
            Self::ListSummaries => serialize(crate::db::list_object_summaries(connection)),
            Self::ListByType(type_id) => {
                serialize(crate::db::list_objects_by_type(connection, &type_id))
            }
            Self::ListRunningTime(source) => serialize(crate::db::list_running_time_entries(
                connection,
                source.as_deref(),
            )),
            Self::ListSummariesByType(type_id) => serialize(
                crate::db::list_object_summaries_by_type(connection, &type_id),
            ),
            Self::EstimateDataVolume(scopes) => {
                serialize(crate::db::estimate_data_volume(connection, &scopes))
            }
            Self::GetByIds(ids) => serialize(crate::db::get_objects_by_ids(connection, &ids)),
            Self::Search(query) => serialize(crate::db::search_objects(connection, &query)),
            Self::Get(id) => serialize(crate::db::get_object(connection, &id)),
            Self::WriteSnapshot(id) => {
                let identity = connection
                    .query_row(
                        "SELECT type_id, type_version FROM objects WHERE id = ?1",
                        [&id],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()
                    .map_err(|error| DbError::Storage(error.to_string()))?;
                let object_revision =
                    crate::db::get_object_revision(connection, &id).map_err(DbError::Storage)?;
                let task_revision = crate::db::task_merge::object_snapshot_token(connection, &id)
                    .map_err(|error| DbError::Storage(error.to_string()))?;
                let snapshot = crate::domain::object_write::ObjectWriteSnapshot {
                    exists: identity.is_some(),
                    type_id: identity.as_ref().map(|value| value.0.clone()),
                    type_version: identity.as_ref().map(|value| value.1.clone()),
                    revision: if identity
                        .as_ref()
                        .is_some_and(|(type_id, _)| type_id == "core.task")
                        || task_revision.is_some()
                    {
                        task_revision.or(object_revision)
                    } else {
                        object_revision
                    },
                };
                serde_json::to_value(snapshot).map_err(|error| DbError::Storage(error.to_string()))
            }
            Self::CanonicalGames(device_id) => serialize(crate::canonical_types::game::list_games(
                connection, &device_id,
            )),
            Self::CanonicalGame(id, device_id) => serialize(
                crate::canonical_types::game::get_game(connection, &id, &device_id),
            ),
            Self::CanonicalAssetSources(object_ids) => serialize(
                crate::canonical_types::facades::asset_sources(connection, &object_ids)
                    .map_err(|error| error.to_string()),
            ),
        }
    }
}
