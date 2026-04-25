# Task: Usage Tracker Uses Ark Core Sync-State Helper

## Context

`services/usage-tracker/src/main.rs` writes usage entities through `ark_core::db` upsert helpers, but still duplicates `lan_sync.version_vector` HLC update logic locally. `ark_core::db::bump_sync_version_vector` now exists as the canonical helper used by ARK RPC local writes.

## Scope

- Replace usage-tracker's local `bump_version_vector` implementation with `ark_core::db::bump_sync_version_vector`.
- Remove now-unused local HLC/version-vector imports and constants.
- Preserve tracker device id storage through `sync_kv`.
- Add/keep tests that prove tracked app ids are stable and local sync version bumps are delegated to Ark core.

## Acceptance Criteria

- AC1: Usage tracker no longer defines local `VERSION_VECTOR_KEY` or local `bump_version_vector`.
- AC2: Tracked app/session/event persistence calls `ark_core::db::bump_sync_version_vector`.
- AC3: Usage tracker still stores/loads `usage_tracker.device_id` through `sync_kv`.
- AC4: Fresh verification passes: `cargo check`, `cargo test`, source grep, and `git diff --check`.
