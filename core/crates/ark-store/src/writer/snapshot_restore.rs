use super::SqliteWriter;
use rusqlite::Connection;

impl SqliteWriter {
    /// Restore a snapshot while the writer actor holds exclusive ownership of
    /// the live connection. The core scheduler controls admission; SQLite
    /// lifecycle and restore mechanics stay inside Store.
    pub fn restore_snapshot(
        &mut self,
        db_path: &str,
        backup_id: &str,
    ) -> Result<crate::snapshot::RestoreReport, String> {
        crate::snapshot::restore_snapshot(&mut self.connection, db_path, backup_id)
    }

    #[cfg(feature = "test-support")]
    pub fn restore_snapshot_with_hooks(
        &mut self,
        db_path: &str,
        backup_id: &str,
        post_verify: impl Fn(&Connection, &[String]) -> Result<(), String>,
        rollback_restore: impl Fn(&mut Connection, &std::path::Path) -> Result<(), String>,
    ) -> Result<crate::snapshot::RestoreReport, String> {
        crate::snapshot::restore_snapshot_with_hooks(
            &mut self.connection,
            db_path,
            backup_id,
            &post_verify,
            &rollback_restore,
        )
    }
}
