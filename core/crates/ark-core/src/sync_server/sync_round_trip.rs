#![allow(clippy::unwrap_used)]
include!("sync_round_trip/fixtures.rs");
include!("sync_round_trip/server_round_trip.rs");
include!("sync_round_trip/authenticated_and_usage.rs");
include!("sync_round_trip/selective_sync.rs");
include!("sync_round_trip/client_discovery.rs");
#[path = "sync_round_trip/policy.rs"]
mod policy;
#[path = "sync_round_trip/six_devices.rs"]
mod six_devices;
