use std::path::Path;

/// Path-based snapshot and integrity helpers for Core service tests.
pub struct SnapshotTestFixture;

impl SnapshotTestFixture {
    /// Creates a legacy database-only snapshot, without publishing a manifest.
    pub fn backup_database(source_path: &Path, destination_path: &Path) -> Result<(), String> {
        let source_path = source_path
            .to_str()
            .ok_or_else(|| "snapshot source path is not UTF-8".to_string())?;
        let destination_path = destination_path
            .to_str()
            .ok_or_else(|| "snapshot destination path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(source_path)?;
        crate::snapshot::backup_connection_to_file(&connection, destination_path)
    }

    pub fn check_integrity(path: &Path) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "integrity database path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        crate::open::check_integrity(&connection)
    }
}
