# @arksync/node Usage API

## Context

`@arksync/node` now exposes sync lifecycle and generic object APIs. Ark usage data still lacks a typed SDK surface, so applications that need tracked apps, sessions, or events are pushed toward direct SQLite access or bespoke wrappers.

## Scope

Add a small typed usage API surface to `@arksync/node` over the existing `ark-core-rpc` operations.

## Acceptance Criteria

- AC1: `@arksync/node` exports TypeScript types for `ArkTrackedAppRecord`, `ArkUsageSessionRecord`, `ArkUsageEventRecord`, and `ArkUsageSnapshot`.
- AC2: `ArkClient` exposes `usage.loadAll()` and it returns only the usage subset from `load_all`.
- AC3: `ArkClient` exposes `usage.trackedApps.upsert/delete` over `upsert_tracked_app` and `delete_tracked_app`.
- AC4: `ArkClient` exposes `usage.sessions.upsert/delete` over `upsert_usage_session` and `delete_usage_session`.
- AC5: `ArkClient` exposes `usage.events.upsert/delete` over `upsert_usage_event` and `delete_usage_event`.
- AC6: Self-managed usage API calls initialize the sidecar DB before the first usage request.
- AC7: Injected `requestFn` mode remains legacy-shaped and sends the expected existing `ark-core-rpc` operation names/payloads.
- AC8: Fresh TypeScript verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- Adding Rust analytics/query endpoints.
- Changing the Ark usage SQLite schema.
- Rewriting Arrancador or Dashboard to consume this API.
- Changing usage-tracker from direct Rust writes to sidecar calls.
