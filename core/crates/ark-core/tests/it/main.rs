//! ark-core integration tests, linked into one test binary.
//!
//! One binary instead of one per file keeps incremental rebuilds and target
//! size down: each file used to link its own copy of ark-core and its
//! dependencies (docs/experiments/2026-09-30-build-speed.md in Cortex). Every
//! test here shares one process, so process-wide state needs a shared guard.

mod ark_api_wire_baseline;
mod data_platform_phase7;
mod fatsecret_type_registration;
mod iroh_bidirectional_burst;
mod iroh_data_admission;
mod iroh_loopback_smoke;
mod iroh_round_trip;
mod iroh_ticket_pairing;
mod phase3_agenda_contract;
mod phase3_blocked_writes;
mod phase3_canonical_types;
mod phase3_compatibility;
mod phase3_delphi_projection;
mod phase3_game_rpc;
mod phase3_registry_definition_authority;
mod phase3_validation;
mod relay_round_trip;
mod support;
