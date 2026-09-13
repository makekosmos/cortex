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
Publish builds additionally require a reviewed release BOM; signing secrets are not read by
the BOM validator or provenance emitter.

## Local checks

```text
# Installed automatically by `bun install`; recover with:
bun run prepare

# Run the same hook contracts manually:
bunx lefthook run pre-commit
bunx lefthook run pre-push

# Full local verification:
bun run check

# Validate a release BOM without building or publishing:
bun run test:release-bom

# Publish a platform build with an explicit, reviewed BOM:
node desktop/scripts/build-desktop.mjs --platform win --bom path/to/release-bom.json
```

`bun run --cwd desktop build` and `build:mac` read the same path from
`KOSMOS_RELEASE_BOM`, so the existing release commands cannot run without an
explicit resolved BOM.

## Shared workspace checkouts

The root `package.json` `kosmos.workspace` object is the single source of truth
for the pinned Imago and arca-sdk commits, package names, versions, and git
integrity values. Core remains derived from `runtime/Cargo.toml`, the ARK
sidecar pin, and the existing `core-pin.yml` toolchain workflow.

```text
# Read-only, pinned by default:
node scripts/workspace.mjs doctor

# Prepare ignored .tmp/workspace checkouts without changing them first:
node scripts/workspace.mjs bootstrap --dry-run
node scripts/workspace.mjs bootstrap

# Explicit local development only; sibling directories are never discovered:
KOSMOS_WORKSPACE_MODE=local \
KOSMOS_IMAGO_PATH=/work/imago \
KOSMOS_ARCA_SDK_PATH=/work/arca-sdk \
node scripts/workspace.mjs doctor
```

Local bootstrap creates the same managed `.tmp/workspace` paths as pinned
mode, using a directory junction on Windows or symlink on Unix. It never
replaces an existing managed checkout or link with the wrong target.

Pinned bootstrap runs each checkout's declared package manager with its frozen
lockfile, builds it, and writes one `.tmp/workspace/prepared.json` stamp with
HEAD, package/lock, and exported-output hashes. Doctor rejects missing or
tampered stamps; matching bootstrap skips the work.

To update a pin, change its commit, package version, and `git:<commit>`
integrity together in `package.json`, then run `doctor` and the release-BOM
tests. Bootstrap never resets, stashes, or overwrites a dirty checkout; remove
`.tmp/workspace` yourself when a prepared checkout is no longer needed.

The publish wrapper validates Cortex/Core commits, the pinned Bun/Node/Rust toolchain, and
the shell/engine/package API contracts before electron-builder starts. It embeds the exact
BOM at `resources/release-bom.json` and emits `release/release-provenance.json` with the
final artifact hashes. A BOM may include expected `artifacts` entries to make a rebuild fail
on a hash or size mismatch; omitted entries are recorded from the final build.

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
