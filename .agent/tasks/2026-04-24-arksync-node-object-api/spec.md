# @arksync/node Object API

## Context

`@arksync/node` currently exposes sync lifecycle helpers but not the generic ARK object model that already exists in `ark-core-rpc`. This keeps applications copying sidecar request wrappers or talking to SQLite directly.

## Scope

Add a small typed object-model API surface to `@arksync/node` over the existing `ark-core-rpc` operations.

## Acceptance Criteria

- AC1: `@arksync/node` exports TypeScript types for `ArkObjectRecord`, `ArkObjectTypeRecord`, `ArkObjectLinkRecord`, and `ArkSearchResult`.
- AC2: `ArkClient` exposes `objects` methods: `list`, `get`, `upsert`, `delete`, and `search`.
- AC3: `ArkClient` exposes `objectTypes` methods: `list`, `get`, `upsert`, and `delete`.
- AC4: `ArkClient` exposes `links` methods: `list`, `upsert`, and `delete`.
- AC5: Self-managed object API calls initialize the sidecar DB before the first object request.
- AC6: Injected `requestFn` mode remains legacy-shaped and sends the expected existing `ark-core-rpc` operation names/payloads.
- AC7: Fresh TypeScript verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- Renaming `@arksync/node` to `@kosmos/ark`.
- Adding usage analytics APIs.
- Changing Rust object schema or RPC operation names.
- Rewriting Eden/Delphi to consume these new namespaces.
