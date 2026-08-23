# Kosmos Shell

This repository owns the desktop shell and its host-side services:

- `desktop/` — Electron host, preload/IPC, packaging and shell UI;
- `manager/` — the standalone Kosmos Manager window;
- `runtime/` — the Rust backend process supervised by the host;
- `native-services/` — Windows focus/watcher services used by the host.

## Current boundary

This repository owns the desktop runtime boundary. Its Rust workspace is
self-contained; ARK is a pinned Git dependency in `runtime/Cargo.toml`.
The desktop scripts fetch and cache the matching `ark-core-rpc` sidecar from
the same Core revision, so a sibling `core` checkout is not required.

Dictation is a separate product in `makekosmos/dictation`; Cortex provides the
host integration it needs at runtime.

## What is intentionally not promised

The repository layout is portable, but the existing desktop build still
expects external extension bundles, ARK, visuals, and release tooling. CI
therefore validates ownership and local metadata only. A green layout check
does not mean that a complete Windows installer can be built here yet.

## Local checks

```text
bun run check:layout
```

After the shared packages are published, the first independent checks should
be `bun run --cwd desktop typecheck`, `bun run --cwd manager typecheck`, and
the focused Rust tests under `runtime/`.
