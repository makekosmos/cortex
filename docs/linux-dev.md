# Linux development: Engine + first-party packages

Verified on Ubuntu 24.04 x86_64. The supported gate builds the Engine (which
hosts ARK in-process) and exercises the Engine HTTP contract in an
isolated `/tmp` root. The whole stack — Engine and the Store catalog gate — is
on `main`; no feature branch is required.

The Linux packaging path does not ship a desktop shell: the Windows installer
is the only supported end-user package. Linux development is therefore Engine
and package-contract development, not Electron/Vue development.

## Toolchain

```text
rustup toolchain install 1.95.0          # workspace requires >= 1.88
sudo apt-get install build-essential pkg-config libssl-dev
node --version                           # Node 20+
corepack prepare pnpm@12.4.1 --activate  # pinned packageManager
```

`pkg-config` + `libssl-dev` are required by `openssl-sys` in the Engine.
`MUNDUS_HEADLESS=1` keeps Engine stubs quiet in tests.

## Engine + ARK

`ark-core` is owned by Cortex and lives under `core/`, without a separate
Git repository or upstream synchronization. Edit and commit Core changes in
Cortex. A single Cortex checkout builds the whole product. `runtime` uses
a path dependency; `node scripts/check-core-pin.mjs` checks that wiring and
rejects a nested `core/.git`.

```text
cargo build -p engine            # writes target/debug/mundus-engine
# ark-core is a library dependency; the Engine serves ARK in-process —
# no sidecar to build or provision.
```

Manual run:

```text
MUNDUS_DATA_DIR=/tmp/mundus-data MUNDUS_HEADLESS=1 target/debug/mundus-engine &
cat /tmp/mundus-data/engine.lock.json    # { pid, http_port, auth_token }
curl -H "Authorization: Bearer <auth_token>" http://127.0.0.1:<http_port>/v1/...
```

Package manifests declare `targets[].os`; the Engine maps every non-Windows
host to `linux`, so a first-party manifest with `"os": ["windows", "linux"]` is
accepted without host changes.

## Engine smoke (one command)

```text
node scripts/linux-smoke.mjs --report .tmp/linux-smoke-report.md
```

Supported flag: `--report FILE`.

`scripts/linux-smoke.mjs` is the reproducible Linux gate for the Engine
contour. Run it from the repository root; it isolates
`XDG_CONFIG_HOME`/`XDG_CACHE_HOME`/`XDG_DATA_HOME` under a per-run `/tmp`
root, then runs the gates in order:

1. `preflight` — Linux plus node/pnpm/cargo/git on `PATH`.
2. `engine-bootstrap` — builds `mundus-engine`, starts the
   Engine against an isolated `MUNDUS_DATA_DIR`, and asserts
   `engine.lock.json` plus authenticated `GET /v1/health` → 200 before
   shutting it down.

Each gate reports `PASS`, `FAIL`, or `NOT_RUN` (a gate is `NOT_RUN` when a
prerequisite gate did not pass). The run exits non-zero unless every gate
passed and always writes a Markdown report — `--report FILE`, defaulting
to `.tmp/linux-smoke-report.md` — with per-gate status, elapsed time, and
a detail line.

Known argv quirk until KOS-111 lands: option values are read positionally
as `argv[indexOf(flag) + 1]`, so an absent flag resolves to `argv[0]` —
the node executable path — rather than `undefined`. Always pass
`--report FILE` explicitly.

## Intentional gaps

- Tray, global shortcuts, dictation capture, foreground-window targeting,
  autostart, and start-menu integration remain Windows-only; the Engine
  stubs them out off-Windows.
- The Arcadia and Dictation package workers are Windows-only: on Linux
  `games.*`/`dictation.trigger` operations answer `unavailable`.

## OS-branch audit (KOS-100)

A KOS-100 audit of first-party app product `src/` (Agenda, Memoria,
Arcadia, Dictation, Store; Ordo was audited before its source moved to
`incubator/ordo-vue`) found no `process.platform` or
`os.platform` use in renderer code. The only app-side OS detection was
window-chrome styling; those sites now consume the Engine-side platform
state instead. Memoria's `getPlatform()` keeps a `navigator.userAgent`
fallback only for sessions running outside a managed host.
Remaining follow-up: Arcadia's library copy is Windows-centric (`.exe`
paths, `C:\Games\...` placeholder). To re-run the search from a directory
holding the app checkouts:

```text
rg -n 'process\.platform|os\.platform|navigator\.platform|navigator\.userAgent|\bwin32\b|\bdarwin\b|data-platform|platform="' \
  agenda/src memoria/src arcadia/src dictation/src
```
