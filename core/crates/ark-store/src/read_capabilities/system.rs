use super::SystemReadCapabilities;
use crate::snapshot::{validate_snapshot, SnapshotValidation};

impl SystemReadCapabilities<'_> {
    pub fn sync_key_value(&self, key: &str) -> Result<Option<String>, String> {
        crate::sync_kv::get_sync_kv(self.connection, key)
    }

    pub fn validate_snapshot(
        &self,
        db_path: &str,
        backup_id: &str,
    ) -> Result<SnapshotValidation, String> {
        validate_snapshot(self.connection, db_path, backup_id)
    }
}
