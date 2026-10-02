# Desktop packaging development

The shipped Mundus Desktop package is Engine + GPUI components. There is no
Electron shell, Vite dev server, or packaged Vue UI in this repository.

Component development happens in the dedicated checkouts (`manager-gpui`,
sibling `agenda-gpui`/`memoria-gpui`/`dictation` repos). This directory owns
packaging scripts and build resources only.

## Building locally

One command builds a complete local installer (Engine + GPUI Manager + NSIS):

```text
pnpm run build:installer:local
```

This runs the same three stages as a release build, but without the preflight,
BOM, receipt, and publish gates:

```text
pnpm --dir desktop run build:backend        # cargo release build + engine payload
pnpm --dir desktop run build:package-components   # GPUI component staging
pnpm run build:desktop -- --local           # NSIS installer (no BOM/publish)
```

`pnpm run build:desktop -- --local` stages the unpacked Engine payload
(`resources\engine\`) and every built GPUI component under
`desktop/.tmp/installer-stage`, compiles
`desktop/build/installer.nsi` with `makensis` (downloaded on demand into
`desktop/.tmp/nsis`), and writes `desktop/release/Mundus-Setup-<ver>.exe` plus
`desktop/release/latest.yml`. It never publishes and never touches the
installed system.

## Release preflight (`pnpm --dir desktop run build`)

`scripts/release-preflight.mjs` gates every release build (KOS-233): it
requires a clean tracked/source worktree, a `main`-HEAD build, a release
version strictly greater than the latest published tag for that platform,
and — for `win` — that `.tmp/engine.next/engine-manifest.json` (built by
`build:backend` from this same tree) reports the same version and commit as
the Desktop release. Release repos per platform live in
`scripts/release-repos.mjs` (win → `makekosmos/cortex`, KOS-304).

The `main`-HEAD and latest-published-tag checks need network access and a
`main` checkout, so they are the only checks skipped with `--local` (or
`MUNDUS_RELEASE_LOCAL=1`) — use that to build and test a candidate locally
from a feature branch, or offline. Everything else (clean worktree, derived BOM,
Engine/Desktop version match) still runs. Never pass `--local` for
a build that will actually be published.
