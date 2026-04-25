# TODO

## Legacy Note

The previous contents of this file described an older `packages/ark/` Python/server-era plan. That is no longer the active ARK architecture.

Current ARK runtime documentation:

- [`packages/ark-core/README.md`](./packages/ark-core/README.md)
- Rust runtime: `packages/ark-core/rust`
- Node/Electron SDK: `packages/kepler-ark` (`@kepler/ark`)
- Compatibility SDK name: `packages/arksync-node` (`@arksync/node`)
- Canonical desktop sidecar: `ark-core-rpc`

## Current ARK Priorities

- Keep app writes going through `ark-core-rpc` / `@kepler/ark`.
- Keep direct Rust writers on `ark_core::db` helpers so sync state is updated consistently.
- Add end-to-end mobile platform smoke tests for relay pairing. Both desktop `ark-core-rpc` and UniFFI `ArkCore::start_sync` now start relay sync when `relay_url` is provided.
- When mobile testing is available, regenerate/consume UniFFI bindings in Android/iOS and verify the mobile apps can pass `relay_url`, `relay_api_key`, and `auth_secret` through `FfiSyncConfig`.
- Add transport encryption before treating LAN sync as secure for sensitive data. LAN sync now has optional HMAC peer authentication, but HMAC does not encrypt payloads.
- Delphi tasks should migrate automatically at app startup into `task_obj` records in the generic object model.
- Eden notes should migrate automatically at app startup into `note_obj` records in the generic object model, while Heart remains available for editor/vault-specific behavior during the transition.
