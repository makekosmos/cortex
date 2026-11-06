//! Engine integration tests, linked into one test binary.
//!
//! One binary instead of one per file roughly halves incremental rebuild time
//! and target size, because each file used to relink the whole engine
//! (docs/experiments/2026-09-30-build-speed.md). Under a plain `cargo test`
//! every test here shares one process, so process-wide state (environment
//! variables, OS keyring entries) is guarded by a shared lock, not a per-module
//! one. The gate runs each test in its own process (cargo-nextest); state
//! shared across processes, such as the OS keyring, is serialized by a test
//! group in .config/nextest.toml.

mod atlas_ops;
mod engine_control_protocol;
