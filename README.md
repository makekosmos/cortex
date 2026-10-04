# CosCast

This repository owns the Mundus desktop application and its host-side services:

- `desktop/` — packaging scripts and resources (no Electron; the shipped
  Windows package is the Engine + the GPUI Manager);
- `manager-gpui/` — the Mundus Manager shipped in the Windows package as
  `resources/components/manager/Mundus Manager.exe`;
- `agenda-gpui` (sibling repo `makekosmos/agenda-gpui`),
  `memoria-gpui` (sibling repo `makekosmos/memoria-gpui`) and
  `dictation` (sibling repo `makekosmos/dictation`) — the native apps the
  Engine's Store installs from their GitHub Releases under
  `%LOCALAPPDATA%\Mundus\Apps`; they are not bundled in the installer
  (KOS-265);
- `runtime/` — the Rust Engine, including privileged operations
  (`mundus-engine privileged …` — managed hosts blocks and fast NTFS
  indexing behind a one-time UAC grant).

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

# Check the release BOM, receipt, and preflight contracts without building:
pnpm run test:release-bom

# Preflight only (must pass before any compilation; derives the BOM from HEAD):
node desktop/scripts/release-preflight.mjs --platform win

# Stage packaged components under desktop/.tmp/components (Windows only):
# each GPUI component is cargo-built for x86_64-pc-windows-msvc and staged as
# <component>/win-unpacked/Mundus <Name>.exe.
node desktop/scripts/build-package-components.mjs

# Build + package + verify (never publishes; writes a receipt):
node desktop/scripts/build-desktop.mjs --platform win

# Run preflight and print the plan without building:
node desktop/scripts/build-desktop.mjs --platform win --dry-run

# Publish only the exact verified receipt (rehashes artifacts and rejects stale/mutated inputs)
# to the makekosmos/cortex releases feed the Engine updater polls:
node desktop/scripts/publish-release.mjs --platform win --receipt desktop/release/release-receipt.v2.json

# Validate publish locally without GH mutation:
node desktop/scripts/publish-release.mjs --platform win --receipt desktop/release/release-receipt.v2.json --dry-run
```

The release BOM (`desktop/scripts/release-bom.mjs`) is never written by hand:
it is derived from the checkout at HEAD — commit, release version, pnpm/Node/Rust
pins, target, and Engine API — and ships as `release-bom.v2.json` next to the
installer. `pnpm --dir desktop run build` runs the whole release build in one go.

The affected-check planner is fail-closed: every mode plans the diff of the
revision under test against `merge-base(HEAD, origin/main)` — staged changes
for `pre-commit`, tracked plus untracked disk changes for the worktree plan,
and pre-push, regardless of whether the remote ref already exists, plans the
union of every pushed commit's diff against its own merge base. A missing
merge base, a shallow clone, a non-commit pushed object, or an empty or
malformed push record selects the full check. Checks always run on the
on-disk tree: when a pushed commit's tree differs from it, the hook prints a
note that the run certified the working tree, not those commits — the push is
not blocked, and the pushed commits are covered by CI. Docs and isolated assets are a no-op, except
documents that tests read as contracts (`DOC_CONTRACTS` in
`scripts/check-plan-manifest.mjs`), which select the checks reading them. A
`package.json` edit that only changes known `"scripts"` entries selects the
checks running those scripts; any other manifest change (dependencies,
engines, packageManager), an unmapped script, or an
unparseable revision selects the full check, as do shared, lockfile, build,
workflow, hook, and unknown changes.

Cheap local baseline: layout 0.108s, source-size test 0.680s, and naming test
0.154s. There is no hosted CI by policy; local Lefthook gates are the only
enforceable checks. Branch protection is disabled, and ruleset/merge-queue
status is NOT_RUN. Use `--full` when reviewing uncertain changes and treat the
planner's `reasons` field as the explanation for a full selection.

The build wrapper runs the release preflight — clean `main`, a version newer than
the latest published release, and an Engine built from the same commit — and derives
the BOM before `makensis` starts. The Windows installer is a
standalone NSIS script (`desktop/build/installer.nsi`) compiled by `makensis`; the build
downloads the pinned NSIS bundle when `MUNDUS_NSIS_DIR` is unset. It emits
`release/release-provenance.json` and atomically writes
`release/release-receipt.v2.json` with exact inputs and final artifact hashes.
`publish-release.mjs` consumes only that receipt; it never builds or packages, and it
re-derives the BOM from HEAD, so a receipt from any other commit is rejected.

`pnpm run check` is `check-plan --full --run`: the full plan's command list in
`scripts/check-plan-commands.mjs` is the single definition of the gate, and
the run records a "full" cache entry only when the tree is unchanged at the
end. It covers layout, source-size, lint, changed-file Oxfmt,
Rustfmt, workspace Clippy with warnings denied, complete workspace
Rust tests, backend tests, and runtime staging. `pnpm install --frozen-lockfile`
installs Lefthook hooks on a clean checkout; run `pnpm run prepare` if hooks are
missing. Pre-commit uses the planner against staged files but runs only the
checks that finish in seconds: Clippy, Rust tests, runtime staging and the
Manager gate are deferred, and a change that selects the full check runs
`pnpm run check:fast` instead (`scripts/check-plan-commit.mjs`). Pre-push
plans the same branch diff as check:affected — `merge-base(HEAD, origin/main)`
to the pushed commit — and caches passes by on-disk tree hash, so a push right
after `pnpm run check:affected` or `pnpm run check` (which records a "full"
entry) skips what already ran; `--full` still runs everything at commit time.

The Rust gate runs every workspace library, integration and Cortex binary test
through [cargo-nextest](https://nexte.st) (`cargo install cargo-nextest
--locked`; the required version is pinned in `.config/nextest.toml`), one
process per test. Doc-tests are disabled on the library targets (`doctest = false` in each crate's manifest): the workspace has none, so the gate does not run an empty `cargo test --doc` pass. Elevated Windows service entrypoints are compiled by Clippy but
not executed by unprivileged hooks; their logic is covered through library tests.
