# Kosmos Shell

This repository owns the desktop shell and its host-side services:

- `desktop/` — Electron host, preload/IPC, packaging and shell UI;
- `manager/` — the standalone Kosmos Manager window;
- `runtime/` — the Rust backend process supervised by the host;
- `native-services/` — Windows focus/watcher services used by the host.

## Current boundary

This is the first ownership split, not yet a standalone installer repository.
The shell currently consumes two shared packages from the Kosmos source tree:
`@kosmos/visuals` and `@kosmos/ark`. Those packages must become versioned
artifacts before this repository can install and build independently.

The runtime also has a source dependency on ARK (`core/ark`). Keep that
dependency explicit while the ARK package contract is being stabilized; do not
copy ARK into this repository.

Dictation remains shell-owned for now: its UI/IPC lives under `desktop/` and
its backend implementation lives under `runtime/`. It is not a separate
repository in this split.

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
