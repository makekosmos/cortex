use super::super::{sealed, CommandClass, CommandContext, DbError, DbQuery};

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
    EstimateDataVolume(ark_application::data_volume::DataVolumeQuery),
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
            Self::LoadAll => serialize(ark_store::load_all::load_all(connection)),
            Self::List => serialize(ark_store::objects::list_objects(connection)),
            Self::ListSummaries => serialize(ark_store::objects::list_object_summaries(connection)),
            Self::ListByType(type_id) => serialize(ark_store::objects::list_objects_by_type(
                connection, &type_id,
            )),
            Self::ListRunningTime(source) => serialize(
                ark_store::objects::list_running_time_entries(connection, source.as_deref()),
            ),
            Self::ListSummariesByType(type_id) => serialize(
                ark_store::objects::list_object_summaries_by_type(connection, &type_id),
            ),
            Self::EstimateDataVolume(scopes) => {
                let session = ark_store::data_volume::SqliteDataVolumeReadSession::new(connection);
                serialize(
                    ark_application::data_volume::DataVolumeReadPort::estimate_data_volume(
                        &session, &scopes,
                    ),
                )
            }
            Self::GetByIds(ids) => {
                serialize(ark_store::objects::get_objects_by_ids(connection, &ids))
            }
            Self::Search(query) => {
                serialize(ark_store::objects::search_objects(connection, &query))
            }
            Self::Get(id) => serialize(ark_store::objects::get_object(connection, &id)),
            Self::WriteSnapshot(id) => serialize(
                ark_store::object_write::read_object_write_snapshot(connection, &id),
            ),
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
