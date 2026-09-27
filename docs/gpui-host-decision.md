# Host: Electron → GPUI transition

**Decision (KOS-145, 2026-09-26; supersedes KOS-131): Kosmos transitions
fully to GPUI. Electron is a departing layer — every capability it still
holds is either already ported, has a named GPUI/Rust destination in the
migration plan below, or is scheduled for deletion. Dual-run was a migration
path, not an end state.**

## § «Why not transition now» — superseded 2026-09-26

The three KOS-131 blockers are resolved or rescoped:

1. **«OS integration lives in Electron»** — rescoped into the concrete
   inventory below. Several items turned out to never be Electron-owned:
   the tray is Rust (`runtime/src/backend_tray`, pinned by
   `desktop/electron/kosmos-tray-contract.test.ts`), the dictation hotkey is
   a Win32 `WH_KEYBOARD_LL` hook in `runtime/src/dictation/hotkey_hook.rs`,
   and the autostart toggle already targets `kepler-backend.exe`, not the
   shell.
2. **«GPUI on Windows is unproven»** — resolved. `manager-gpui` and
   `agenda-gpui` ship inside the Windows installer as
   `components/{manager,agenda}` and run against the production Engine;
   `dictation-gpui` (repo `makekosmos/dictation`) ships a standalone
   always-on-top overlay driven by Engine WS events.
   `kosmos-gpui-kit` carries the shared theme/widgets/Engine client
   (including the WS subscriber that was the last dictation blocker).
3. **«Risk profile differs by layer»** — the Electron surface has already
   been cut down: Launcher, Vue Dashboard, Vue Manager, Focus/Pomodoro UI
   and the `.kext` extension runtime are deleted (ledger in
   `docs/repo-split-decisions.md`). What remains is inventoried below.

## What GPUI already covers

- `manager-gpui` — the full Manager surface over `/v1/rpc`: Настройки
  (engine autostart toggle via `engine.autostart.*`, dictation hotkey
  capture, update state), Обновления, Usage (`get_usage_analytics` +
  `app_index.*`), Data object browser, Secrets, Sync, Connections, Dev,
  About. Packaged as `components/manager/Kosmos Manager.exe`.
- `agenda-gpui` — packaged as `components/agenda/Kosmos Agenda.exe`.
- `memoria-gpui` — packaged as `components/memoria/Kosmos Memoria.exe`,
  pinned via `desktop/component-pins.json` (`memoria_gpui`) and launched
  through the shell command «Открыть Memoria (GPUI)», the Start Menu
  «Kosmos Memoria» shortcut, or the GPUI Manager About view; the Vue
  Memoria package (`com.kosmos.memoria` .kspkg) stays installable as
  fallback (KOS-156).
- `dictation-gpui` — standalone GPUI app owning the pill overlay and
  session orchestration; singleton via mutex; driven by Engine WS
  `dictation_toggle_trigger` / `dictation_ptt_trigger` events.
- Engine-side services are shell-agnostic by construction: tray icon/menu,
  dictation audio capture/transcription/injection, `engine.autostart.*`,
  package launch leases (`launch_id` + `broker_token` + TTL renewal),
  user-data roots, `engine.lock.json` discovery under `KOSMOS_DATA_DIR`.

## Inventory — what Electron still holds

Rebuilt from `desktop/electron/*` and `host/electron/*` (2026-09-26), not
from memory. «Window» means a real `BrowserWindow` creation site.

### A. Root process & lifecycle (`Kosmos.exe`, `desktop/electron/`)

| Capability | Files | Notes |
|---|---|---|
| Instance slot system | `instance.ts`, `data-dir.ts` | `prod`/`dev`/`dev-<x>`/`test-<x>` slots → userData dir, ARK `dataDir`, `productName`, Windows AUMID, per-slot hotkey/autoupdater/autorun gating. Also Kepler→Kosmos userData + settings migration and `verifyUserDataMatches` boot self-check. |
| Single-instance & argv routing | `main.ts` | `requestSingleInstanceLock`; second-instance args: `--autostart` (silent), `--kosmos-update-check`, `--kosmos-update-install`, default → `openManager()`. |
| Boot orchestration | `main.ts`, `main-app-ready.ts` | `nativeTheme` dark, spawn backend, open surface, `powerMonitor` resume → backend recovery, `will-quit` cleanup. macOS Chromium occlusion switches. |
| Engine supervision | `main-backend-bootstrap.ts`, `main-backend-process.ts`, `main-backend-supervisor.ts`, `main-ark-client-controller.ts` | Backend exe resolution (`KEPLER_BACKEND_EXE` → `target/{debug,release}` → installed Engine `current.json` pointer under `%LOCALAPPDATA%\Kosmos\Engine` → packaged `Kosmos Runtime.exe`); detached spawn with env contract (`KOSMOS_DATA_DIR`, `KEPLER_INSTANCE`, desktop-authority credential/PID, `KOSMOS_TRAY_ICON` seeding, test-mode flags); `--restart-core`; `engine.lock.json` liveness; ArkClient init with retries and `awaitArkReady`; renderer event broadcast (`kepler:backend:*`, `kepler:ark:event`). |
| Crash reporting | `main.ts`, `main-crashes-ipc.ts` | `crashReporter` local minidumps (`uploadToServer: false`), `render-process-gone`/`child-process-gone` logging, crash-file list/delete IPC. |
| Logging & diagnostics | `logging.ts`, `redaction.ts`, `diagnostics.ts`, `diagnostics-window-benchmark.ts` | JSONL logs under `<dataDir>/logs/<slot>/`, support ZIP bundle (7d logs, 30d crashes, versions, extensions list, redacted), window-move benchmark harness. |

### B. OS integration

| Capability | Files | Notes |
|---|---|---|
| Login items / autostart | `settings-autostart.ts`, `settings-autostart-controller.ts` | `app.setLoginItemSettings` + `getLoginItemSettings` for «Kosmos Engine» (`kepler-backend.exe --start`), incl. `launchItems` matching; legacy cleanup across `CosCast`/`Kepler`/`Kosmos*` names + `reg.exe` HKCU `Run` delete. Prod-slot gated. The Engine-side ops (`engine.autostart.*`) already exist — this is the legacy-name hygiene layer. |
| Global shell hotkey | `main-app-ready.ts`, `settings-window.ts`, `settings-store.ts` | `globalShortcut` `Alt+Space` (prod) / `Alt+\`` (dev) → `openHostedApp("com.kosmos.shell")`; 80 ms anti-repeat; re-register on settings change; persisted in `kosmos-settings.json`; F12 DevTools in dev. |
| Auto-updater | `autoupdater-host.ts`, `main-data-ipc-settings.ts`, `release-bom-identity.ts` | `electron-updater` state machine (checking → available → downloading → downloaded/error), broadcast to all windows + persisted `<dataDir>/update-state.json` (consumed by `manager-gpui` via `KOSMOS_UPDATE_STATE_FILE`), native MessageBox fallback after 5 min, `quitAndInstall`; triggers: once per start, second-instance flags, settings IPC. GitHub-release feeds `makekosmos/desktop` / `desktop-mac`; embeds `release-bom.json` digest. |
| Native dialogs | `main-data-ipc.ts`, `autoupdater-host.ts`, `full-access-consent-ipc.ts`, `main-backend-bootstrap.ts`, `host/electron/main.ts` | `showOpenDialog` (export dir, file-index root, hosted-app directory grants), `showMessageBox` (update install prompt, full-access consent warning), `showErrorBox` (boot failures, hosted-app launch failure). |
| OS notifications | `system-notifications.ts` | `Notification` toast wrapper. **Orphaned** — last callers left with the Focus/Pomodoro UI removal; no in-tree importer remains. |
| Shell integration | `main-shell-ipc.ts`, `host-app.ts` | `shell.openExternal`; `shell.openPath` on Start-Menu `.lnk` fallback. |
| Window chrome helpers | `mac-window.ts`, `window-effects.ts` | macOS vibrancy + centered traffic lights; Win32 acrylic/mica/`backgroundMaterial` env resolution. |
| Tray | — | **Not Electron.** Owned by `runtime/src/backend_tray` (contract test asserts `new Tray(` never appears in `main.ts`); Electron only seeds `tray.ico` via `KOSMOS_TRAY_ICON`. |

### C. Windows (BrowserWindow sites)

| Window | Files | Status |
|---|---|---|
| Settings | `settings-window.ts`, `settings-store.ts`, `settings-storage-summary.ts` | Vue hash-route `#settings`; IPC for hotkey/tray-flag/version/storage summary. Superseded by `manager-gpui` Настройки (engine autostart + dictation hotkey already live there). |
| Dictation pill overlay | `dictation-pill.ts`, `main-runtime-integrations.ts` | Transparent always-on-top `screen-saver`-level overlay, warmup, `kepler:dictation:*` IPC, Engine `dictation_*_trigger` event subscription. **Superseded by `dictation-gpui`** — this is the departing implementation still wired into `main.ts`. |
| Focus-mode windows | — | **Already deleted** (D2, 2026-09-25): FocusWidget/BlockOverlay/focus-session windows are gone. What remains is OS-level blocking: `focus-block.ts` (spawn `Kosmos Helper.exe` / UAC `RunAs` fallback / named-pipe service client for hosts-file edits) and `focus-enforcement.ts` (authoritative apply after `focus.set_active_state` on the `kepler:ark:request` path). |
| Test harness | `main-test-window.ts` | Hidden Playwright window (`KOSMOS_TEST_MODE`). |

### D. Renderer contract (dies with the last Vue surface)

| Capability | Files |
|---|---|
| Preload bridges | `preload.ts`, `preload-bridge.ts`, `preload-settings-bridge.ts` (`window.kepler` API) |
| Custom protocols | `main-protocols.ts`, `app-icon-protocol.ts`, `shared/electron/local-image-protocol.ts` — `kosmos-icon://app/<id>` (Engine `app_index.icon_path` + byte cache), `kosmos-local-image://` |
| Command registry | `commands.ts`, `main-commands.ts` — static commands (open GPUI Agenda, open GPUI Memoria, open hosted graph, dictation toggle, settings, check-updates) merged with Engine `commands.list` |
| Data IPC | `main-data-ipc.ts`, `main-data-ipc-settings.ts`, `main-shell-ipc.ts` — `kepler:ark:request` passthrough (with `assertMainRendererArkRequestAllowed` consent guard), search/objects, export list/run, settings + update + test hooks |
| Full-access consent | `full-access-consent.ts` (live guard), `full-access-consent-ipc.ts` (dialog flow — currently orphaned, was wired through the removed extension host) |
| Component spawners | `manager-navigation.ts` (`Kosmos Manager.exe`), `agenda-navigation.ts` (`Kosmos Agenda.exe`), `memoria-navigation.ts` (`Kosmos Memoria.exe`), `host-app.ts` `openHostedApp()` (`Kosmos Package Host.exe --open-app=<id>`, `KOSMOS_HOST_*` dev paths, Start-Menu `.lnk` fallback) |

### E. Legacy data migration

| Capability | Files | Notes |
|---|---|---|
| `.kext` → `.kspkg` migrator | `legacy-migration-journal.ts`, `legacy-migration-state.ts`, `legacy-migration-runtime.ts`, `legacy-migration-recovery.ts`, `extension-data-migration.ts`, `test-migration-*` | Journaled (prepared → finalizing → committed) id + extension-data merge, boot-time run after Ark ready, crash recovery. Allowlist: arcadia/eden/delphi → canonical ids. One-shot by design. |

### F. Package Host (`host/electron/` — `Kosmos Package Host.exe`)

| Capability | Files | Notes |
|---|---|---|
| Entry & routing | `host/electron/main.ts`, `app-navigation.ts` | `--open-app=<id>` argv, single-instance lock, second-instance open routing, dev `--dev-url`. |
| Hosted-package windows | `main.ts` (`openApp`) | One BrowserWindow per launched `.kspkg` app at `manifest.launch_url`; title/icon pinning; per-app window registry. This is the «BrowserWindow for hosted packages / `openHostedPackage`» item — the public entry point is `openHostedApp` in `host-app.ts`. |
| Launch leases | `launch-ownership.ts`, `main.ts` | `LaunchOwnership` claims per app id; v2 `broker_token` + TTL renewal loop (`scheduleLaunchRenewal`); revoke on last-window close, on navigation leaving the launch origin (`will-navigate`/`did-start-navigation`), and bounded drain on quit; replaced-claim adoption by surviving aux windows. The lease protocol itself is Engine-defined and shell-agnostic. |
| Auxiliary windows («stickers») | `app-windows.ts`, `main.ts` (`openAuxWindow`) | Renderer-keyed secondary windows, ≤8 per app, persisted bounds + `alwaysOnTop` under `userData/window-states/`, pin IPC. |
| Permission brokers | `main.ts` IPC + `brokers/*`, `host-api.ts` | `host:ark-request` (v2 launch-scoped broker token / v1 capability scopes), `host:ark-subscribe` event fan-out (read-permission filtered; `db_restored` → window reload), `host:launcher-request` allowlist (`app_index.*`, `file_index.*`, `commands.*`), `host:user-data` legacy FS store + `extension-user-data-ipc.ts` (Engine user-data roots), `host:dialogs:pick-directory-grant`, `host:app-open`, `host:window:{open,always-on-top}` + minimize/close. |
| Host lifecycle & glue | `lifecycle.ts`, `shortcuts.ts`, `kosmos-app-branding.ts`, `preload.ts` | Warm-exit timer (Engine `getWarmTimeout`, 0\|300 s), `Ctrl+Shift+K` globalShortcut → `com.kosmos.shell`, Start-Menu `.lnk` reconcile for installed apps, per-app icon/name branding, `window.kosmosApp` preload. |

## Ordered migration plan

Ordered leaf-first: peripheral capabilities move before the orchestrator
that spawns them. Steps 1–4 are already landed and listed for the record.

1. ✅ **Manager → `manager-gpui`** (KOS-134). Vue `manager/` deleted.
2. ✅ **Agenda → `agenda-gpui`** packaged component (KOS-137).
3. ✅ **Dictation pill → `dictation-gpui`** standalone app (KOS-137);
   the Electron pill path (`dictation-pill.ts`) is now removal work.
4. ✅ **Launcher / Focus UI / Vue Dashboard / `.kext` runtime deleted**
   (KOS-137 ledger).
5. **Shell settings → Engine-owned state.** Move `kosmos-settings.json`
   keys (shell hotkey, tray-icon flag) to Engine storage or a settings op;
   port storage summary and app version surfaces into `manager-gpui`
   Настройки; delete `settings-window.ts` + `settings-store.ts` +
   `settings-storage-summary.ts` + the `kepler:settings:*` IPC block.
6. **Shell entry path.** Replace `globalShortcut` `Alt+Space` →
   `openHostedApp("com.kosmos.shell")` (and `Ctrl+Shift+K` inside the
   Package Host) with a GPUI-owned entry: the runtime already has the
   low-level hook machinery (`hotkey_hook.rs`); tray click already works
   via the Rust tray. Decides what «open the shell» means post-launcher.
7. **Auto-updater → shell-agnostic.** `electron-updater` is the largest
   Electron-only dependency left. Move feed check + download into the
   runtime or installer tooling, keeping the `update-state.json` contract
   `manager-gpui` already reads; delete `autoupdater-host.ts` and the
   `--kosmos-update-*` second-instance verbs (re-route them to the GPUI
   root process in step 12).
8. **Small OS surfaces → Rust/GPUI equivalents.** Native dialogs
   (`rfd`/GPUI prompt — incl. the full-access consent dialog, which becomes
   an Engine-mediated approval), `shell.openExternal` (`open` crate /
   `ShellExecute`), notifications (Engine-owned toast op or GPUI shell —
   also deletes the orphaned `system-notifications.ts`), crash collection
   (minidump handler in Rust or keep the existing `crashes/` dir contract),
   diagnostics bundle (pure fs/zip logic — ports directly).
9. **Hosted packages → off the Electron Package Host.** The hard block.
   Two compatible tracks: (a) each active `.kspkg` app gets a native GPUI
   port (agenda done; memoria packaged/launchable as of KOS-156;
   arcadia/ordo pending), and (b) if a
   transitional web-rendered host is needed, it must not be Electron —
   but the direction is per-app ports, not a new webview layer. The
   launch-lease protocol (`launch_id`/`broker_token`/TTL renew/revoke) is
   Engine-defined and carries over unchanged; `LaunchOwnership`, aux
   windows, brokers and shortcut reconcile are reimplemented in Rust or
   deleted with the apps that needed them. `host/electron/` retires when
   the last hosted Vue package leaves it.
10. **Focus OS-blocking → Engine/native services.** `focus-block.ts` +
    `focus-enforcement.ts` are spawn/IPC plumbing around
    `Kosmos Helper.exe` and the focus service — the trigger belongs behind
    `focus.set_active_state` in the Engine path, not behind
    `kepler:ark:request` in a shell.
11. **Legacy migrator → Engine or sunset.** The `.kext`→`.kspkg` journaled
    migration is ArkClient + fs work; run it as an Engine-side startup
    task (or accept it as finished and delete) so the Electron entry point
    stops owning it.
12. **Root process → GPUI.** The last structural step: single-instance
    (mutex — pattern already proven in `dictation-gpui`), argv routing
    (`--autostart`, `--open-app`, update verbs), Engine spawn/supervision
    (exe resolution, env contract, `--restart-core`, lock liveness,
    power-resume recovery) and shutdown orchestration move into the GPUI
    root binary — the same process that already embeds the Manager
    surface.
13. **Packaging & distribution.** electron-builder/NSIS is replaced by an
    installer that puts the GPUI exe at the root and keeps
    `components/*` unchanged; repoint Start-Menu shortcuts, AUMID,
    autostart entry and the update feed; keep signing + one-revision BOM.
14. **Deletion sweep.** `desktop/electron/*`, `host/electron/*`,
    `desktop/src` (Vue renderer), preload bridges, Playwright harnesses
    that only exercise the Electron surface, `electron`/`electron-updater`/
    `electron-builder` deps, `desktop/package.json` product config.
15. **userData sunset.** Migrate anything still relevant out of Electron
    `userData` (`%APPDATA%\Kosmos App*`): settings (already Engine-side by
    step 5), `window-states/` (dies with aux windows or moves to Engine
    data dir), Crashpad (retired). Nothing else is worth moving.

## What stays regardless

- Engine is the only state owner; every shell (Electron today, GPUI next)
  uses `/v1/rpc` + `engine.lock.json` discovery. No shell opens SQLite.
- The one-revision BOM (`release-bom.mjs`) covers Host + kepler-backend +
  ark-core-rpc from a single commit — independent of shell choice.
- `KOSMOS_DATA_DIR` + `KEPLER_INSTANCE` slot isolation remains the instance
  contract for every shell, GPUI included.
- Hosted-app authority stays Engine-issued: launch leases, broker tokens
  and permission grants are Engine contracts, not Electron features.
