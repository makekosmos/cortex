# CosCast

This repository owns the CosCast desktop application and its host-side services:

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
# Installed automatically by `bun install`; recover with:
bun run prepare

# Run the same hook contracts manually:
bunx lefthook run pre-commit
bunx lefthook run pre-push

# Full local verification:
bun run check
```

`bun run check` covers layout, source-size, lint, changed-file Oxfmt, all frontend
typechecks, Rustfmt, workspace Clippy with warnings denied, complete workspace
Rust tests, backend tests, and runtime staging. `bun install --frozen-lockfile`
installs Lefthook hooks on a clean checkout; run `bun run prepare` if hooks are
missing. Pre-commit uses `glob_matcher: doublestar` to limit frontend typechecks
to the changed Desktop, Host, or Manager tree; pre-push intentionally runs the
full backend and runtime suite.

The Rust gate runs every workspace library and integration test plus Cortex
binary tests. Elevated Windows service entrypoints are compiled by Clippy but
not executed by unprivileged hooks; their logic is covered through library tests.
