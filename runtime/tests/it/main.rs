//! Engine integration tests, linked into one test binary.
//!
//! One binary instead of one per file roughly halves incremental rebuild time
//! and target size, because each file used to relink the whole engine
//! (docs/experiments/2026-09-30-build-speed.md). The price is that every test
//! here shares one process: state that is process-wide (environment variables,
//! the worker failure hooks, OS keyring entries) must be guarded by a shared
//! lock, not a per-module one.

mod ark_markdown_bridge_worker;
mod cortex_2_acceptance;
mod desktop_authority_socket;
mod dictation_worker_contract_windows;
mod engine_control_protocol;
mod integration_replication_consumer;
mod integration_replication_headless;
mod integration_replication_hpke;
mod package_worker_authority;
mod package_worker_process_windows;
mod package_worker_windows;
mod phase5_runtime_grants;
mod snapshot_registry_fds;
mod store_catalog_runtime;

// Shared by the integration_replication_* tests.
#[allow(clippy::unwrap_used)]
#[path = "support/integration_replication_offline.rs"]
mod offline;
#[allow(clippy::unwrap_used)]
#[path = "support/integration_replication_setup.rs"]
mod support;
