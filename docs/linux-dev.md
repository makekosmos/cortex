# Linux development: host + Engine + Manager + first-party apps

Verified on Ubuntu 24.04 x86_64 (KOS-53, KOS-93, KOS-94, KOS-95, KOS-96,
KOS-97). The supported gate is the first-party E2E suite, which builds the
Engine, spawns the pinned `ark-core-rpc` sidecar, installs the signed
Agenda/Memoria/Ordo/Arcadia/Dictation `.kspkg`, and launches the Electron
Host under Xvfb — plus the standalone Manager E2E suite, which attaches the
Manager Electron app to a live Engine over `engine.lock.json` — all in
isolated `/tmp` roots with PID-identity cleanup.

## Toolchain

```text
rustup toolchain install 1.95.0          # workspace requires >= 1.88
sudo apt-get install build-essential pkg-config libssl-dev xvfb
node --version                           # Node 20+
corepack prepare pnpm@12.4.1 --activate  # pinned packageManager
```

`pkg-config` + `libssl-dev` are required by `openssl-sys` in the Engine.
`xvfb` provides the display; `KOSMOS_HEADLESS=1` keeps windows hidden in tests.

## Engine + ARK sidecar

```text
cargo build -p kepler-backend            # writes target/debug/kepler-backend
cargo build --bin ark-core-rpc           # writes target/debug/ark-core-rpc
```

The Engine discovers the sidecar via `ARK_CORE_RPC_PATH` or workspace
candidates; the E2E harness passes an explicit path. Manual run:

```text
KOSMOS_DATA_DIR=/tmp/kosmos-data KOSMOS_HEADLESS=1 target/debug/kepler-backend &
cat /tmp/kosmos-data/engine.lock.json    # { pid, http_port, auth_token }
curl -H "Authorization: Bearer <auth_token>" http://127.0.0.1:<http_port>/v1/...
```

Package manifests declare `targets[].os`; the Engine maps non-Windows/non-macOS
to `linux`, so a first-party manifest with `"os": ["windows", "linux"]` is
accepted without host changes.

## Host dependencies

`host/package.json` depends on `@makekosmos/ark` from GitHub Packages, so the
normal path needs a token with `read:packages`:

```text
NODE_AUTH_TOKEN=<token with read:packages> pnpm --dir host install --frozen-lockfile
```

If the token cannot read private packages, fall back to the pinned workspace
checkout (`node scripts/workspace.mjs bootstrap` provisions
`.tmp/workspace/arca-sdk`), temporarily pointing the dependency at it with
`link:.tmp/workspace/arca-sdk` — do not commit that edit.

```text
pnpm --dir host run typecheck
pnpm --dir host run build                # writes host/dist-electron
```

## E2E (headless)

```text
xvfb-run -a pnpm --dir host run e2e \
  first-party-agenda-contract.spec.ts first-party-agenda-smoke.spec.ts \
  first-party-memoria-contract.spec.ts first-party-memoria-smoke.spec.ts \
  first-party-memoria-import-crash.spec.ts \
  first-party-ordo-contract.spec.ts \
  first-party-arcadia-contract.spec.ts \
  first-party-dictation-contract.spec.ts
```

- `first-party-agenda-contract` — signed Agenda installs, launches in Host,
  performs scoped ARK reads/writes through `window.kosmosApp.ark`, denies
  out-of-grant operations, and survives an Engine restart with persisted data.
- `first-party-agenda-smoke` — the daily smoke: packaged Agenda UI capture
  ("Новая задача") → row in Входящие (Inbox) → task card, with the object
  verified through the ARK bridge.
- `first-party-memoria-contract` — signed Memoria installs, rolls back a
  partial Obsidian-vault import, performs scoped ARK CRUD through
  `window.kosmosApp.ark`, denies out-of-grant operations, and survives an
  Engine crash + restart with persisted data.
- `first-party-memoria-smoke` — the daily smoke: packaged Memoria UI
  "Добавить заметку" → title + body typed in the Tiptap editor → autosave →
  card in Всё (Everything), with the note verified through the ARK bridge.
- `first-party-memoria-import-crash` — an import interrupted after a durable
  entry write rolls back on the next Engine/Host launch.
- `first-party-ordo-contract` — signed Ordo installs, launches in Host, runs
  scoped focus/pomodoro operations through `window.kosmosApp.ark`
  (blocklist upsert/list/resolve, focus activate, pomodoro
  start/pause/resume/stop), denies out-of-grant operations, and keeps
  pomodoro + focus state across Engine and Host restarts. Requires the
  reviewed `release/ordo-0.1.3.kspkg` built in the `ordo/` checkout at the
  pinned commit (`bun install && bun run package:kspkg`).
- `first-party-arcadia-contract` — signed Arcadia installs and launches in
  Host through the `kosmos-host` manifest target (the only target declared
  for Linux; the `worker` target stays Windows-only because package workers
  are unsupported off Windows). The Linux smoke exercises the typed
  `com.kosmos.game@1.0.0` grant through `window.kosmosApp.ark`:
  `upsert_object` create+update, `get_object`/`list_objects` read-back,
  out-of-grant denials (`upsert_object_type`, foreign `com.kosmos.note`
  write, `delete_object`), `games.*` answering `unavailable` without a
  worker, and the object surviving an Engine crash + restart. Requires the
  reviewed `release/arcadia-0.1.11.kspkg` built in the `arcadia/` checkout
  at the pinned commit (`bun install && bun run package:kspkg`). On Windows
  the same spec additionally covers the worker path: `games.*` CRUD, the
  hostile-Steam launch rejection, and SQOBA backup/restore recovery.
- `first-party-dictation-contract` — signed Dictation installs and launches
  in Host through the `kosmos-host` manifest target (the `worker` target
  stays Windows-only because package workers are unsupported off Windows).
  The Linux smoke exercises the Engine-owned `dictation.*` grant through
  `window.kosmosApp.ark`: `dictation.get_state`/`get_config`/
  `list_local_models`/`update_config`, the recording state machine
  (`start_recording` → `recording`, duplicate → `unavailable`, `cancel` →
  `idle`), out-of-grant denial (`dictation.submit_audio` →
  `invalid-request`), sanitized responses (no path/key-shaped keys reach the
  renderer), and config surviving an Engine restart. The
  `dictation.control`/`worker.invoke` ops the Linux manifest declares —
  `dictation.capture.start`, `dictation.window.foreground`,
  `dictation.lifecycle.set_autostart`, `dictation.trigger` — answer
  `unavailable` because capture/hotkey/autostart are Engine stubs off
  Windows and no package worker is running; the spec asserts that rather
  than branching on the OS. Requires the reviewed
  `release/dictation-0.2.5.kspkg` built in the `dictation/` checkout at the
  pinned commit (`bun install && bun run package:kspkg`). On Windows the
  same spec replays the checked-in `dictation-0.2.2` fixture, which predates
  the worker contract.

## Manager E2E (headless)

```text
pnpm --dir manager run build            # writes manager/dist + dist-electron
xvfb-run -a pnpm --dir manager run e2e  # all specs; append spec names to filter
```

`manager/scripts/run-e2e.mjs` wraps Playwright with a cleanup manifest: each
spec runs in a `kosmos-manager-e2e-*` temp root, and the runner sweeps
leftover roots/processes by PID identity (`/proc` start time on Linux,
WMI `CreationDate` on Windows). `manager/e2e/global-setup.ts` builds the
Engine + `ark-core-rpc` sidecar with the shared test signing keys before the
suite starts. On Linux the specs export `ELECTRON_DISABLE_SANDBOX=1` and
isolate `XDG_CONFIG_HOME`/`APPDATA` per run; `KOSMOS_HEADLESS=1` keeps the
Manager window hidden (asserted via `BrowserWindow.isVisible() === false`).

- `diagnostics` — Engine up; Manager launches, walks every sidebar section
  (Данные, Синхронизация, Движок, Интеграции, Ключи, Браузер, Маркетплейс,
  Обновления, О приложении, Настройки), and exercises the metadata-only
  diagnostics IPC surface: crash reports expose names only (never file
  contents or paths), support-bundle save cancels headless-safe, and
  folder-open calls answer `opened: false` off a real display.
- `engine-lifecycle` — the attach/reattach smoke: Engine writes
  `engine.lock.json` (api v1), Manager attaches and answers
  `getHealth`/`getInfo`/`getEngineSettings`, exits independently while the
  Engine stays alive, and a reattached Manager re-reads the same lock and
  increments the Engine's `protocol-usage.json` `api_v1` counters. Also
  covers retained dictation assets across migration and — on Node 22.5+
  runners where `node:sqlite` exists — the legacy usage-tracker settings
  migration (skipped on Node 20).
- `file-index-manager` — `file_index.*` Engine RPCs and the
  `window.kosmosManager.*FileIndex*` API keep working across a Manager
  relaunch; the File Index feature intentionally has no sidebar surface.
- `sync-manager` — a deterministic in-process HTTP fixture answers the same
  `engine.lock.json` + `/v1/rpc` contract: sync snapshot, pairing-code
  reveal/copy, connect/disconnect peer flows, serialized refresh, and an
  unavailable-Engine reopen. The lock `auth_token` is asserted absent from
  the rendered DOM.

The harness is platform-neutral: binary names come from `executableName()`,
fixture ZIPs use `desktop/scripts/zip-utils.mjs`, and process cleanup reads
`/proc` on Linux (PID + start-time identity) while Windows keeps the
PowerShell/WMI path. Linux Electron containers typically lack the namespaces
the Chromium sandbox needs, which the specs disable only on
`process.platform === "linux"` (`--no-sandbox` in Host specs,
`ELECTRON_DISABLE_SANDBOX=1` in Manager specs).

## Intentional gaps

- `pnpm install` for `host/` requires `read:packages` on GitHub Packages;
  without it use the `link:` fallback above.
- The Arcadia and Dictation package workers are Windows-only: on Linux
  `games.*`/`dictation.trigger` operations answer `unavailable`, and the
  Steam-scan/SQOBA save-backup flows are exercised by the Arcadia spec only
  on Windows. The Linux-built `release/arcadia-0.1.11.kspkg` and
  `release/dictation-0.2.5.kspkg` still carry the declared Windows worker
  entries (`worker/arcadia-worker.exe`, `worker/dictation-worker.exe`, ELF
  binaries there) because the packager always copies the built worker to
  that path — they are never launched off Windows.
- Building the Arcadia/Dictation packages on Linux needs their
  `@kosmos/visuals` dependency resolved to a sibling `imago/` checkout
  (the vite config aliases it) — install/build imago first, then
  `bun run package:kspkg` in `arcadia/` or `dictation/`.
- Specs for other first-party apps (Daedalus/Shell/topology)
  assume the monorepo layout or Windows-only tools and are not part of
  the Linux gate.
- Tray, global shortcuts, dictation capture, foreground-window targeting,
  autostart, and start-menu integration remain Windows-only; the Engine
  stubs them out off-Windows and the Dictation spec asserts `unavailable`
  rather than the app branching on the OS.
- Manager keeps the same rule: `manager.getAutostart` reports
  `available: false` for the unpackaged e2e build (and for anything not
  `win32`), and Manager `src/` contains no `process.platform` checks —
  capability decisions stay in the Engine/main-process responses. The
  Manager suite asserts `available: false` instead of adding UI branches.
- `manager/e2e` is not covered by `pnpm --dir manager run typecheck`
  (`tsconfig` includes `src` + `electron` only, same as `host/`); the specs
  are exercised by Playwright instead.
- The usage-tracker settings-migration test needs `node:sqlite` (Node
  22.5+); on Node 20 runners it skips, so the row-count assertions are
  Windows/CI-only for now.
- The bundled Manager CSP (`font-src 'self'`) blocks the app's own `data:`
  fonts — a pre-existing product issue; the diagnostics spec ignores
  exactly that console error and still fails on any other.
