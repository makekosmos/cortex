# Desktop development

KOS-10 dev runs are isolated under `desktop/.e2e/runs/<run-id>`.

```text
bun run --cwd desktop dev:no-build       # start Vite + Electron
bun run --cwd desktop dev:stop           # stop the latest owned run
bun run --cwd desktop dev:reset          # stop and remove only that run
bun run --cwd desktop dev:diagnostics    # print its manifest
bun run --cwd desktop dev:smoke          # live dev smoke, then clean up
```

`dev:no-build` never invokes Cargo. Runtime or contract changes require a
fresh `bun run --cwd desktop build:backend:dev` (or the existing prepared
artifact/build flow); UI-only edits reuse the running Vite process.

Each run has a unique data directory, Electron user-data directory, shell
port, manifest, renderer log, and shell log. Stop validates the recorded PID
start time and run marker, then terminates only that process tree. Reset
resolves every path and refuses anything outside the recognized test root.

Packaged smoke requires an explicit `KOSMOS_PACKAGED_ROOT` containing
`Kosmos.exe`, `resources/Kosmos Engine.zip`, and its matching
`resources/engine-manifest.json`. It verifies and installs that archive under
the isolated run root; it never falls back to the user's installed engine.
Missing production fixture assets are reported as `NOT_RUN`. Dev and packaged
smoke use the same backend-ready IPC assertion.

Set `KOSMOS_SMOKE_LIVE_DNS=1` to add the real
`dictation.test_connectivity` ARK round-trip and require its `dns_resolve`
stage to succeed. The default smoke remains deterministic and offline-safe.

## Release preflight (`pnpm run build` / `build:mac`)

`scripts/release-preflight.mjs` gates every release build (KOS-233): it
requires a clean tracked/source worktree, a `main`-HEAD build, a release
version strictly greater than the latest published tag for that platform,
and — for `win` — that `.tmp/engine.next/engine-manifest.json` (built by
`build:backend` from this same tree) reports the same version and commit as
the Desktop release.

The `main`-HEAD and latest-published-tag checks need network access and a
`main` checkout, so they are the only checks skipped with `--local` (or
`KOSMOS_RELEASE_LOCAL=1`) — use that to build and test a candidate locally
from a feature branch, or offline. Everything else (clean worktree, BOM, ARK
artifact, Engine/Desktop version match) still runs. Never pass `--local` for
a build that will actually be published.
