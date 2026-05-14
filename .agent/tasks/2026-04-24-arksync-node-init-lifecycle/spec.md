# @arksync/node Init Lifecycle

## Context

`@arksync/node` can spawn `ark-core-rpc` when no injected `requestFn` is provided, but the package currently calls `start_sync` without first initializing the sidecar database. `ark-core-rpc` requires an `init` request with a database path before sync operations.

Delphi injects an existing sidecar request function, so that mode must keep using the already-managed sidecar and must not spawn or initialize a second process.

## Scope

Add the smallest standalone lifecycle support to `@arksync/node` so self-managed clients can provide a `dbPath` and have `start()` initialize `ark-core-rpc` before starting sync.

## Acceptance Criteria

- AC1: `ArkClientOptions` accepts a `dbPath?: string` option documented as required when `requestFn` is not provided.
- AC2: In self-managed mode, `ArkClient.start()` sends exactly one `init` request with `dbPath` before the first `start_sync` request.
- AC3: In injected `requestFn` mode, `ArkClient.start()` does not require `dbPath` and does not send `init`.
- AC4: If no `requestFn` and no `dbPath` are provided, `start()` fails with a clear `@arksync/node` error before attempting `start_sync`.
- AC5: TypeScript verification for `packages/arksync-node` passes and raw command evidence is recorded.

## Out Of Scope

- Renaming `@arksync/node` to `@kosmos/ark`.
- Adding object CRUD or usage APIs to the SDK.
- Changing the `ark-core-rpc` wire protocol.
- Adding request ids or multiplexing.
