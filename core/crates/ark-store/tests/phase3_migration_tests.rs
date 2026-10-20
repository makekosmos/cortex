#![allow(clippy::unwrap_used)]

#[path = "phase3_blocked_writes.rs"]
mod blocked_writes;
#[path = "phase3_migration_ledger.rs"]
mod ledger;
#[path = "phase3_migration.rs"]
mod migration;
#[path = "phase3_migration_objects.rs"]
mod objects;
#[path = "phase3_migration_orchestrator.rs"]
mod orchestrator;
#[path = "phase3_migration_preflight.rs"]
mod preflight;
#[path = "phase3_migration_registry.rs"]
mod registry;
#[path = "phase3_migration_system.rs"]
mod system;

#[path = "support/phase3_legacy_fixtures.rs"]
mod phase3_legacy_fixtures;

mod support;
