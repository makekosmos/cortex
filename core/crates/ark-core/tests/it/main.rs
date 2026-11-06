//! ark-core integration tests, linked into one test binary.
//!
//! One binary instead of one per file keeps incremental rebuilds and target
//! size down: each file used to link its own copy of ark-core and its
//! dependencies (docs/experiments/2026-09-30-build-speed.md in Cortex). Every
//! test here shares one process, so process-wide state needs a shared guard.

mod agenda_write_payload_contract;
mod ark_api_wire_baseline;
mod canonical_validation;
mod fatsecret_type_registration;
mod game_rpc_events;
mod iroh_bidirectional_burst;
mod iroh_data_admission;
mod iroh_loopback_smoke;
mod iroh_round_trip;
mod iroh_ticket_pairing;
mod registry_definition_authority;
mod relay_round_trip;
mod selective_sync_projection;
