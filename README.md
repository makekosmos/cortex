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
# Installed automatically by `bun install`; recover with:
bun run prepare

# Full local verification:
bun run check
```

`bun run check` covers layout, source-size, lint, changed-file Oxfmt, all frontend
typechecks, Rustfmt, workspace Clippy with warnings denied, backend tests, and
runtime staging. `bun install --frozen-lockfile` installs Lefthook hooks on a
clean checkout; run `bun run prepare` if hooks are missing.
