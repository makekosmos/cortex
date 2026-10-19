use crate::application::usage::UsageWriteSession;
use crate::db;
use crate::types::{UsageDay, UsageEvent, UsageSession, UsageSpanWrite};
use rusqlite::Transaction;

pub(crate) struct SqliteUsageWriteSession<'tx, 'connection> {
    transaction: &'tx Transaction<'connection>,
}

impl<'tx, 'connection> SqliteUsageWriteSession<'tx, 'connection> {
    pub(crate) fn new(transaction: &'tx Transaction<'connection>) -> Self {
        Self { transaction }
    }
}

impl UsageWriteSession for SqliteUsageWriteSession<'_, '_> {
    fn ensure_sequence_migrated(&self, device_id: &str) -> Result<(), String> {
        db::ensure_usage_sequence_migrated(self.transaction, device_id)
    }

    fn upsert_session(&self, session: &UsageSession) -> Result<(), String> {
        ark_store::usage::upsert_usage_session(self.transaction, session)
    }

    fn delete_session(&self, id: &str) -> Result<(), String> {
        ark_store::usage::delete_usage_session(self.transaction, id)
    }

    fn upsert_event(&self, event: &UsageEvent) -> Result<(), String> {
        ark_store::usage::upsert_usage_event(self.transaction, event)
    }

    fn delete_event(&self, id: &str) -> Result<(), String> {
        ark_store::usage::delete_usage_event(self.transaction, id)
    }

    fn upsert_span(&self, span: &UsageSpanWrite) -> Result<Vec<UsageDay>, String> {
        ark_store::usage_queries::upsert_usage_span(self.transaction, span)
    }

    fn record_revision(
        &self,
        entity_type: &str,
        id: &str,
        device_id: &str,
        deleted: bool,
    ) -> Result<String, String> {
        let hlc =
            db::bump_sync_version_vector(self.transaction, entity_type, id, device_id, deleted)?;
        if deleted {
            ark_store::sync_tombstones::record(self.transaction, entity_type, id, None, &hlc)?;
        } else {
            ark_store::sync_tombstones::delete(self.transaction, id)?;
        }
        Ok(hlc)
    }
}
