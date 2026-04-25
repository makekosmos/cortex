# @arksync/node README

## Context

`@arksync/node` has moved beyond a sync-only helper. It now owns sidecar init lifecycle, request ids for self-managed sidecars, sync lifecycle, generic objects, object types, links, and usage entity APIs. The package currently has no README, so consumers have no canonical integration path.

## Scope

Add a focused README for `packages/arksync-node` that documents the current supported API and its limits.

## Acceptance Criteria

- AC1: `packages/arksync-node/README.md` exists.
- AC2: The README documents the two lifecycle modes: self-managed sidecar and injected sidecar request function.
- AC3: The README documents that self-managed mode requires `dbPath` and runs `init` automatically before sync/object/usage calls.
- AC4: The README documents relay as explicitly unsupported by `@arksync/node`/`ark-core-rpc` today.
- AC5: The README includes short examples for sync start, object API, object type/link API, and usage API.
- AC6: The README warns consumers not to write directly to Ark SQLite from Electron apps and to use the SDK/Rust APIs instead.
- AC7: Fresh documentation verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- Rewriting the root repository README.
- Renaming the package.
- Publishing package metadata.
- Changing SDK behavior.
