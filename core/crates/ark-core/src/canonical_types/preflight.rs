use ark_application::source_snapshot::SourceSnapshot;
use rusqlite::Connection;

pub use ark_application::migration_source_projection::{
    BlockedItem, MigrationPreflightReport, PreflightType, SourceKind, SourceRecord,
};

pub fn inventory_sources(conn: &Connection) -> Result<Vec<SourceRecord>, String> {
    let snapshot = ark_store::source_snapshot::read_source_snapshot(conn)?;
    ark_application::migration_source_projection::inventory_snapshot(&snapshot)
}

pub fn preflight_phase3(conn: &Connection) -> Result<MigrationPreflightReport, String> {
    let snapshot: SourceSnapshot = ark_store::source_snapshot::read_source_snapshot(conn)?;
    let context = ark_application::migration_source_projection::mapping_context(&snapshot)?;
    ark_application::migration_source_projection::preflight_from_snapshot(&snapshot, &context)
}
