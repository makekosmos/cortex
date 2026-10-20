mod inventory;
mod native_sources;
mod report;

pub use inventory::{
    BlockedItem, MigrationPreflightReport, PreflightType, SourceKind, SourceRecord,
};
pub use native_sources::inventory_snapshot;
pub use report::preflight_from_snapshot;
