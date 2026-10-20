use std::path::Path;

use rusqlite::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageRollbackState {
    pub usage_days: i64,
    pub sync_heads: i64,
    pub sync_log: i64,
    pub sync_versions: i64,
}

/// Reads the persistence state covered by Core's invalid usage write test.
pub struct UsageRollbackFixture;

impl UsageRollbackFixture {
    pub fn inspect(path: &Path, device_id: &str) -> Result<UsageRollbackState, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "usage rollback path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        let usage_days = count(
            &connection,
            "SELECT COUNT(*) FROM usage_days WHERE device_id = ?1",
            device_id,
        )?;
        let sync_heads = count(
            &connection,
            "SELECT COUNT(*) FROM usage_sync_heads WHERE device_id = ?1",
            device_id,
        )?;
        let sync_log = count(
            &connection,
            "SELECT COUNT(*) FROM usage_sync_log WHERE device_id = ?1",
            device_id,
        )?;
        let sync_versions = count(
            &connection,
            "SELECT COUNT(*) FROM usage_sync_versions WHERE entity_id LIKE ?1",
            &format!("usage-day:{device_id}:%"),
        )?;
        Ok(UsageRollbackState {
            usage_days,
            sync_heads,
            sync_log,
            sync_versions,
        })
    }
}

fn count(connection: &Connection, sql: &str, value: &str) -> Result<i64, String> {
    connection
        .query_row(sql, [value], |row| row.get(0))
        .map_err(|error| error.to_string())
}
