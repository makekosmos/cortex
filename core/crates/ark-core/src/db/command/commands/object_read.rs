use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};

fn serialize<T: serde::Serialize>(value: Result<T, String>) -> Result<serde_json::Value, DbError> {
    let value = value.map_err(DbError::Storage)?;
    serde_json::to_value(value).map_err(|error| DbError::Storage(error.to_string()))
}

#[derive(serde::Serialize)]
pub(crate) enum ObjectReadQuery {
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

impl StoreReadQuery for ObjectReadQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        match self {
            Self::List => serialize(capabilities.objects().list()),
            Self::ListSummaries => serialize(capabilities.objects().list_summaries()),
            Self::ListByType(type_id) => serialize(capabilities.objects().list_by_type(&type_id)),
            Self::ListRunningTime(source) => {
                serialize(capabilities.objects().list_running_time(source.as_deref()))
            }
            Self::ListSummariesByType(type_id) => {
                serialize(capabilities.objects().list_summaries_by_type(&type_id))
            }
            Self::EstimateDataVolume(query) => {
                serialize(capabilities.objects().estimate_data_volume(&query))
            }
            Self::GetByIds(ids) => serialize(capabilities.objects().get_by_ids(&ids)),
            Self::Search(query) => serialize(capabilities.objects().search(&query)),
            Self::Get(id) => serialize(capabilities.objects().get(&id)),
            Self::WriteSnapshot(id) => serialize(capabilities.objects().write_snapshot(&id)),
            Self::CanonicalGames(device_id) => {
                serialize(capabilities.canonical().games(&device_id))
            }
            Self::CanonicalGame(id, device_id) => {
                serialize(capabilities.canonical().game(&id, &device_id))
            }
            Self::CanonicalAssetSources(object_ids) => serialize(
                capabilities
                    .canonical()
                    .asset_sources(&object_ids)
                    .map_err(|error| error.to_string()),
            ),
        }
    }
}
