use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};

fn serialize<T: serde::Serialize>(value: Result<T, String>) -> Result<serde_json::Value, DbError> {
    let value = value.map_err(DbError::Storage)?;
    serde_json::to_value(value).map_err(|error| DbError::Storage(error.to_string()))
}

#[derive(serde::Serialize)]
pub(crate) enum UsageReadQuery {
    TitleTotal(String),
    Analytics {
        range_days: i64,
        top_apps_limit: i64,
        recent_sessions_limit: i64,
        windows_dir: Option<String>,
    },
    RecentProcesses(i64),
    SearchProcesses {
        query: String,
        limit: i64,
    },
    GamePlaytimeSummary {
        bindings: Vec<crate::types::UsageGamePlaytimeBinding>,
        range_start: Option<String>,
        range_end: Option<String>,
    },
}

impl sealed::Sealed for UsageReadQuery {}

impl StoreReadQuery for UsageReadQuery {
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
            Self::TitleTotal(query) => serialize(capabilities.usage().title_total(&query)),
            Self::Analytics {
                range_days,
                top_apps_limit,
                recent_sessions_limit,
                windows_dir,
            } => serialize(capabilities.usage().analytics(
                range_days,
                top_apps_limit,
                recent_sessions_limit,
                windows_dir.as_deref(),
            )),
            Self::RecentProcesses(limit) => serialize(capabilities.usage().recent_processes(limit)),
            Self::SearchProcesses { query, limit } => {
                serialize(capabilities.usage().search_processes(&query, limit))
            }
            Self::GamePlaytimeSummary {
                bindings,
                range_start,
                range_end,
            } => serialize(capabilities.usage().game_playtime_summary(
                &bindings,
                range_start.as_deref(),
                range_end.as_deref(),
            )),
        }
    }
}
