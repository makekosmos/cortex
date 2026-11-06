use super::UsageReadCapabilities;
use ark_domain::types::{UsageProcessCandidate, UsageTimeBinding};

impl UsageReadCapabilities<'_> {
    pub fn title_total(&self, query: &str) -> Result<ark_domain::types::UsageTitleTotal, String> {
        crate::usage_queries::get_usage_title_total(self.connection, query)
    }

    pub fn analytics(
        &self,
        range_days: i64,
        top_apps_limit: i64,
        recent_sessions_limit: i64,
        windows_dir: Option<&str>,
    ) -> Result<ark_domain::types::UsageAnalyticsSnapshot, String> {
        crate::usage_queries::load_usage_analytics(
            self.connection,
            range_days,
            top_apps_limit,
            recent_sessions_limit,
            windows_dir,
        )
    }

    pub fn recent_processes(&self, limit: i64) -> Result<Vec<UsageProcessCandidate>, String> {
        crate::usage_queries::list_recent_usage_processes(self.connection, limit)
    }

    pub fn search_processes(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<UsageProcessCandidate>, String> {
        crate::usage_queries::search_usage_processes(self.connection, query, limit)
    }

    pub fn usage_summary(
        &self,
        bindings: &[UsageTimeBinding],
        range_start: Option<&str>,
        range_end: Option<&str>,
    ) -> Result<ark_domain::types::UsageTimeSummary, String> {
        crate::usage_queries::load_usage_summary(self.connection, bindings, range_start, range_end)
    }
}
