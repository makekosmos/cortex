# ARK P2P Work Progress

## Legacy Note

The previous contents of this file described an older TypeScript `packages/arksync/` / `@arksync/core` extraction plan. That is no longer the active integration contract.

Current ARK runtime documentation:

- [`packages/ark-core/README.md`](./packages/ark-core/README.md)
- Rust runtime: `packages/ark-core/rust`
- Node/Electron SDK: `packages/kosmos-ark` (`@kosmos/ark`)
- Compatibility SDK name: `packages/arksync-node` (`@arksync/node`)
- Canonical desktop sidecar: `ark-core-rpc`

## Current Status

- LAN discovery and WebSocket sync live in `packages/ark-core/rust`.
- `ark-core-rpc` owns the desktop sidecar lifecycle.
- `@kosmos/ark` is the supported TypeScript integration layer for Electron main processes. `@arksync/node` is compatibility-only.
- Relay options now start the relay sync bridge in both `ark-core-rpc` and UniFFI `ArkCore::start_sync`; mobile platform smoke tests still need a dedicated pass.
- LAN sync now supports optional HMAC authentication on `hello` messages via `auth_secret`; payload encryption is still future work.
