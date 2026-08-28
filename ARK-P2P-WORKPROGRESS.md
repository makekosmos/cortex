# ARK P2P Work Progress

## Legacy Note

The previous contents of this file described an older TypeScript `packages/arksync/` / `@arksync/core` extraction plan. That is no longer the active integration contract.

Current ARK runtime documentation:

- [`crates/ark-core/README.md`](./crates/ark-core/README.md)
- Rust runtime: `crates/ark-core`
- Generated TypeScript bindings: `core/ark/packages/ark/src/generated`
- Canonical desktop sidecar: `ark-core-rpc`

## Current Status

- LAN discovery and WebSocket sync live in `crates/ark-core`.
- `ark-core-rpc` owns the desktop sidecar lifecycle.
- Cortex consumes `ark-core` from its pinned Core Git revision and stages `ark-core-rpc` for Desktop.
- Relay options now start the relay sync bridge in both `ark-core-rpc` and UniFFI `ArkCore::start_sync`; mobile platform smoke tests still need a dedicated pass.
- LAN sync now supports optional HMAC authentication on `hello` messages via `auth_secret`; payload encryption is still future work.
