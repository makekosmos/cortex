use super::super::{sealed, CommandClass, CommandContext, DbError, DbQuery};

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

impl DbQuery for UsageReadQuery {
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
            Self::TitleTotal(query) => serialize(ark_store::usage_queries::get_usage_title_total(
                connection, &query,
            )),
            Self::Analytics {
                range_days,
                top_apps_limit,
                recent_sessions_limit,
                windows_dir,
            } => serialize(ark_store::usage_queries::load_usage_analytics(
                connection,
                range_days,
                top_apps_limit,
                recent_sessions_limit,
                windows_dir.as_deref(),
            )),
            Self::RecentProcesses(limit) => serialize(
                ark_store::usage_queries::list_recent_usage_processes(connection, limit),
            ),
            Self::SearchProcesses { query, limit } => serialize(
                ark_store::usage_queries::search_usage_processes(connection, &query, limit),
            ),
            Self::GamePlaytimeSummary {
                bindings,
                range_start,
                range_end,
            } => serialize(ark_store::usage_queries::load_usage_game_playtime_summary(
                connection,
                &bindings,
                range_start.as_deref(),
                range_end.as_deref(),
            )),
        }
    }
}
