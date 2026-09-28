# CosCast

This repository owns the Mundus desktop application and its host-side services:

- `desktop/` — packaging scripts and resources (no Electron; the shipped Windows package is Engine + GPUI components);
- `manager-gpui/` — the Mundus Manager shipped in the Windows package as
  `resources/components/manager/Mundus Manager.exe`;
- `agenda-gpui` (sibling repo `makekosmos/agenda-gpui`, pinned in
  `desktop/component-pins.json`) — the Agenda shipped in the Windows
  package as `resources/components/agenda/Agenda.exe`;
- `memoria-gpui` (sibling repo `makekosmos/memoria-gpui`, pinned in
  `desktop/component-pins.json`) — the Memoria shipped in the
  Windows package as `resources/components/memoria/Memoria.exe`;
- `dictation` (sibling repo `makekosmos/dictation`, pinned in
  `desktop/component-pins.json`) — the Dictation shipped in the
  Windows package as `resources/components/dictation/Dictation.exe`;
- `runtime/` — the Rust backend process supervised by the Engine;
- `native-services/` — Windows focus/watcher services used by the Engine.

## Current boundary

This repository owns the desktop runtime boundary. Its Rust workspace is
self-contained; `ark-core` lives in-tree under `core/crates/ark-core` and the
Engine hosts it in-process, so a sibling `core` checkout is not required.

The shipped Mundus Desktop for Windows contains no Electron. The product is
the Mundus Engine (`mundus-engine` with in-process ARK, tray + updater +
autostart) plus GPUI components under `resources/components/<name>/`.

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
# Installed automatically by `pnpm install`; recover with:
pnpm run prepare

# Run the same hook contracts manually:
pnpm exec lefthook run pre-commit
pnpm exec lefthook run pre-push

# Full local verification:
pnpm run check

# Print the affected-check plan (JSON on stdout, explanation on stderr):
pnpm run check:plan

# Execute only the selected local checks:
pnpm run check:affected

# Force the conservative full path:
node scripts/check-plan.mjs --full --run

# Validate a release BOM without building or publishing:
pnpm run test:release-bom

# Preflight only (must pass before any compilation):
node desktop/scripts/release-preflight.mjs --platform win --bom path/to/release-bom.json

# Stage packaged components under desktop/.tmp/components (Windows only):
# each GPUI component is cargo-built for x86_64-pc-windows-msvc and staged as
# <component>/win-unpacked/Mundus <Name>.exe.
MUNDUS_RELEASE_BOM=path/to/release-bom.json node desktop/scripts/build-package-components.mjs

# Build + package + verify (never publishes; writes a receipt):
node desktop/scripts/build-desktop.mjs --platform win --bom path/to/release-bom.json

# Run local preflight and print the plan without building or contacting GitHub:
node desktop/scripts/build-desktop.mjs --platform win --bom path/to/release-bom.json --dry-run

# Publish only the exact verified receipt (rehashes artifacts and rejects stale/mutated inputs):
node desktop/scripts/publish-release.mjs --platform win --receipt desktop/release/release-receipt.v1.json

# Validate publish locally without GH mutation or HTTP:
node desktop/scripts/publish-release.mjs --platform win --receipt desktop/release/release-receipt.v1.json --dry-run
```

`pnpm run --cwd desktop build` and `build:mac` read the same path from
`MUNDUS_RELEASE_BOM`, so the existing release commands cannot run without an
explicit resolved BOM.

## Shared workspace checkouts

The root `package.json` `mundus.workspace` object is the single source of truth
for the pinned Imago and arca-sdk commits, package names, versions, and git
integrity values. Core remains derived from `runtime/Cargo.toml`, the ARK
sidecar pin, and the root `toolchain.json` toolchain pins.

```text
# Read-only, pinned by default:
node scripts/workspace.mjs doctor

# Prepare ignored .tmp/workspace checkouts without changing them first:
node scripts/workspace.mjs bootstrap --dry-run
node scripts/workspace.mjs bootstrap

# Explicit local development only; sibling directories are never discovered:
MUNDUS_WORKSPACE_MODE=local \
MUNDUS_IMAGO_PATH=/work/imago \
MUNDUS_ARCA_SDK_PATH=/work/arca-sdk \
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

The affected-check planner is fail-closed: staged changes use `pre-commit`,
the worktree plan includes tracked and untracked files, and pre-push input
uses the pushed ref range. Missing, invalid, zero, shallow, or ambiguous
revisions select the full check. Docs and isolated assets are a no-op;
shared, lockfile, manifest, build, workflow, hook, and unknown changes select
the full check.

Cheap local baseline: layout 0.108s, source-size test 0.680s, and naming test
0.154s. There is no hosted CI by policy; local Lefthook gates are the only
enforceable checks. Branch protection is disabled, and ruleset/merge-queue
status is NOT_RUN. Use `--full` when reviewing uncertain changes and treat the
planner's `reasons` field as the explanation for a full selection.

The build wrapper validates Cortex/Core commits, the pinned pnpm/Node/Rust toolchain, and
the engine/package API contracts before `makensis` starts. The Windows installer is a
standalone NSIS script (`desktop/build/installer.nsi`) compiled by `makensis`; the build
downloads the pinned NSIS bundle when `MUNDUS_NSIS_DIR` is unset. It emits
`release/release-provenance.json` and atomically writes
`release/release-receipt.v1.json` with exact inputs and final artifact hashes.
`publish-release.mjs` consumes only that receipt; it never builds or packages. A BOM may
include expected `artifacts` entries to make a rebuild fail on a hash or size mismatch;
omitted entries are recorded from the final build.

`pnpm run check` covers layout, source-size, lint, changed-file Oxfmt,
Rustfmt, workspace Clippy with warnings denied, complete workspace
Rust tests, backend tests, and runtime staging. `pnpm install --frozen-lockfile`
installs Lefthook hooks on a clean checkout; run `pnpm run prepare` if hooks are
missing. Pre-commit uses the planner against staged files; pre-push uses the
Git-provided ref range when stdin is available and otherwise falls back to the
full backend and runtime suite.

The Rust gate runs every workspace library and integration test plus Cortex
binary tests. Elevated Windows service entrypoints are compiled by Clippy but
not executed by unprivileged hooks; their logic is covered through library tests.
