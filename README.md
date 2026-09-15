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

See [`docs/rust-build-cache.md`](docs/rust-build-cache.md) for Rust cache,
sidecar reuse, prebuilt UI development, and baseline evidence.

```text
# Installed automatically by `bun install`; recover with:
bun run prepare

# Run the same hook contracts manually:
bunx lefthook run pre-commit
bunx lefthook run pre-push

# Full local verification:
bun run check

# Print the affected-check plan (JSON on stdout, explanation on stderr):
bun run check:plan

# Execute only the selected local checks:
bun run check:affected

# Force the conservative full path:
node scripts/check-plan.mjs --full --run

# Validate a release BOM without building or publishing:
bun run test:release-bom

# Preflight only (must pass before any compilation):
node desktop/scripts/release-preflight.mjs --platform win --bom path/to/release-bom.json

# Build + package + verify (never publishes; writes a receipt):
node desktop/scripts/build-desktop.mjs --platform win --bom path/to/release-bom.json

# Run local preflight and print the plan without building or contacting GitHub:
node desktop/scripts/build-desktop.mjs --platform win --bom path/to/release-bom.json --dry-run

# Publish only the exact verified receipt (rehashes artifacts and rejects stale/mutated inputs):
node desktop/scripts/publish-release.mjs --platform win --receipt desktop/release/release-receipt.v1.json

# Validate publish locally without GH mutation or HTTP:
node desktop/scripts/publish-release.mjs --platform win --receipt desktop/release/release-receipt.v1.json --dry-run
```

`bun run --cwd desktop build` and `build:mac` read the same path from
`KOSMOS_RELEASE_BOM`, so the existing release commands cannot run without an
explicit resolved BOM.

The affected-check planner is fail-closed: staged changes use `pre-commit`,
the worktree plan includes tracked and untracked files, pre-push input uses
the pushed ref range, and CI uses its explicit base/head SHAs. Missing,
invalid, zero, shallow, or ambiguous revisions select the full check. Docs and
isolated assets are a no-op; shared, lockfile, manifest, build, workflow, hook,
and unknown changes select every CI job. The `cortex-quality-gate` job remains
required even when selected jobs are intentionally skipped.

Cheap local baseline: layout 0.108s, source-size test 0.680s, and naming test
0.154s. Full local and hosted job-minute measurements are NOT_RUN; hosted CI
was blocked by KOS-50. Branch protection is disabled, and ruleset/merge-queue
status is NOT_RUN. Use `--full` when reviewing uncertain changes and treat the
planner's `reasons` field as the explanation for a full selection.

The build wrapper validates Cortex/Core commits, the pinned Bun/Node/Rust toolchain, and
the shell/engine/package API contracts before electron-builder starts. Every local
electron-builder path uses `--publish never`. It embeds the exact BOM at
`resources/release-bom.json`, emits `release/release-provenance.json`, and atomically
writes `release/release-receipt.v1.json` with exact inputs and final artifact hashes.
`publish-release.mjs` consumes only that receipt; it never builds or packages. A BOM may
include expected `artifacts` entries to make a rebuild fail on a hash or size mismatch;
omitted entries are recorded from the final build.

`bun run check` covers layout, source-size, lint, changed-file Oxfmt, all frontend
typechecks, Rustfmt, workspace Clippy with warnings denied, complete workspace
Rust tests, backend tests, and runtime staging. `bun install --frozen-lockfile`
installs Lefthook hooks on a clean checkout; run `bun run prepare` if hooks are
missing. Pre-commit uses the planner against staged files; pre-push uses the
Git-provided ref range when stdin is available and otherwise falls back to the
full backend and runtime suite.

The Rust gate runs every workspace library and integration test plus Cortex
binary tests. Elevated Windows service entrypoints are compiled by Clippy but
not executed by unprivileged hooks; their logic is covered through library tests.
