//! ark-core integration tests, linked into one test binary.
//!
//! One binary instead of one per file keeps incremental rebuilds and target
//! size down: each file used to link its own copy of ark-core and its
//! dependencies (docs/experiments/2026-09-30-build-speed.md in Cortex). Every
//! test here shares one process, so process-wide state needs a shared guard.

mod data_platform_phase7;
mod data_platform_tombstones;
mod fatsecret_type_registration;
mod integration_schema_upgrade;
mod iroh_bidirectional_burst;
mod iroh_bidirectional_network;
mod iroh_loopback_smoke;
mod iroh_round_trip;
mod iroh_ticket_pairing;
mod phase2_slice2_object_versions;
mod phase3_canonical_facades;
mod phase3_canonical_types;
mod phase3_compatibility;
mod phase3_delphi_projection;
mod phase3_game_rpc;
mod phase3_migration;
mod phase3_migration_ledger;
mod phase3_migration_objects;
mod phase3_migration_orchestrator;
mod phase3_migration_preflight;
mod phase3_migration_registry;
mod phase3_migration_system;
mod phase3_validation;
mod proptest_invariants;
mod relay_round_trip;
mod relay_transport_events;
mod sync_round_trip;
mod type_registry_slice1;

// Shared by the phase3_migration* tests.
#[allow(clippy::unwrap_used)]
#[path = "support/phase3_legacy_fixtures.rs"]
mod phase3_legacy_fixtures;
