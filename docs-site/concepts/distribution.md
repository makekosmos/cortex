# Distribution

::: tip TL;DR
- **Kepler launcher** дистрибутится через `yoso-industries/kepler-releases` (NSIS installer + electron-updater).
- **Расширения** дистрибутятся через `yoso-industries/kosmos-extensions` (per-extension tagged releases + `catalog.json`).
- **Source code** остаётся в `ksanrse/kepler` monorepo. Distribution repos — binary only.
:::

## Двухрепозиторная Raycast-style модель

```
ksanrse/kepler (private monorepo)         ← source of truth
        │
        │  bun run --cwd shell build
        ↓                                  bun run --cwd shell ext:publish <id>
yoso-industries/kepler-releases             yoso-industries/kosmos-extensions
   v0.1.0/                                    horologion-v0.3.0/
     Kepler-Setup-0.1.0.exe                     horologion-0.3.0.kext
     latest.yml                               delphi-v0.2.0/
                                              ...
                                            catalog.json (auto-regenerated)
        │                                          ↑
        │  HTTPS (electron-updater)                │ HTTPS (catalog fetch)
        ↓                                          │
   End-user Kepler.exe ───────────────────────────┘
```

Launcher и marketplace полностью независимы — пользователь обновляет
лаунчер и расширения отдельно.

## Kepler launcher: autoUpdater

Реализация — `shell/electron/autoupdater-host.ts` (state machine поверх
`electron-updater`). Вызывается из `shell/electron/main.ts` как
`setupAutoUpdater({ isDev })`.

- Skip в dev mode (`isDev: true`, передаётся из main.ts когда
  `VITE_DEV_SERVER_URL` set) и test mode (`KOSMOS_TEST_MODE=1`).
- `check()` на старте + каждые **6 часов** (`setInterval`).
- `autoDownload = true`, `autoInstallOnAppQuit = false` — установку драйвит
  пользователь кликом по banner'у.
- `update-downloaded` → broadcast `kind: "downloaded"`. Native dialog
  «Перезапустить сейчас?» появляется как **fallback через 5 минут**, если
  пользователь не нажал banner.

Конфиг publish'а — в `shell/package.json → build.publish[0]` (provider github,
owner yoso-industries, repo kepler-releases).

### State machine

```
idle → checking → (available → downloading → downloaded) | not-available | error
any → checking (manual or periodic)
```

`UpdateState` (см. `autoupdater-host.ts`):

| kind | поля |
|---|---|
| `idle` | — |
| `checking` | — |
| `not-available` | `checkedAt: number` |
| `available` | `version: string` |
| `downloading` | `version: string`, `percent: number` |
| `downloaded` | `version: string` |
| `error` | `message: string` |

Broadcast идёт на IPC канал **`kepler:settings:update:state`** для **всех**
`BrowserWindow`'ов (launcher / settings / extensions). UI слушает и
ререндерит banner.

### Public API (main process)

```ts
import {
  setupAutoUpdater,
  check,
  install,
  getState,
} from "./autoupdater-host";

setupAutoUpdater({ isDev });   // init + первый check + 6h interval
await check();                  // manual force-check (idempotent)
install();                      // quitAndInstall — для click-to-install
getState();                     // текущий UpdateState (snapshot)
```

### IPC handlers / renderer API

| IPC channel | direction | payload |
|---|---|---|
| `kepler:settings:update:check` | invoke | → `UpdateState` |
| `kepler:settings:update:install` | invoke | → `void` |
| `kepler:settings:update:state` | invoke | → `UpdateState` (snapshot) |
| `kepler:settings:update:state` | event (main→renderer) | `UpdateState` (push) |

Renderer (через preload `window.kepler.settings.update`):

```ts
await window.kepler.settings.update.check();
await window.kepler.settings.update.install();
const state = await window.kepler.settings.update.state();
const off = window.kepler.settings.update.onStateChanged((s) => {
  // re-render banner
});
```

### Raycast-style banner UI

`shell/src/views/SettingsView.vue` рендерит sticky banner высотой 32px
сверху Settings окна, если `state.kind !== "idle"` и `!= "not-available"`.

| State | Текст | Icon | Поведение клика |
|---|---|---|---|
| `available` | «Обновление Kepler X.Y.Z — нажми чтобы скачать» | `ArrowUpCircle` | autoDownload уже идёт, click no-op (или повторный check) |
| `downloading` | progress bar + `{percent}%` | `Loader2` (spin) | disabled |
| `downloaded` | «Обновление готово — нажми чтобы установить и перезапустить» | `ArrowUpCircle` | `install()` → quitAndInstall |
| `error` | сообщение error'а | — | retry check |

Иконки — `lucide-vue-next`.

### Manual «Проверить обновления»

Доступно двумя способами:

1. **Settings → General → кнопка «Проверить обновления»** — вызывает
   `window.kepler.settings.update.check()`.
2. **Launcher команда `kepler:check-updates`** (Ctrl+Shift+K → «Проверить
   обновления»). Хендлер `runCheckUpdates()` в `shell/electron/commands.ts`
   только вызывает `check()` из autoupdater-host — окно настроек не
   открывается. Обновлённый state приходит в launcher через update banner
   (pinned tile в секции «Обновление») и параллельно в Settings, если оно
   уже открыто.

### Tray icon в production

В production окно tray грузит иконку из `process.resourcesPath/icon.png`.
`shell/electron/main.ts → createTray()` пробует candidate-paths по
приоритету:

1. `<process.resourcesPath>/icon.png` (production, кладётся через
   `extraResources` в `shell/package.json → build`).
2. `<__dirname>/../build/icon.png` (dev fallback).
3. `<__dirname>/../../build/icon.png` (dev из dist-electron).

Без этого в production install tray icon был чёрным placeholder'ом —
`__dirname` указывал на `<install>/resources/app.asar/dist-electron/` и
относительный путь не резолвился.

### Релиз launcher'а

```powershell
# Один раз: $env:GH_TOKEN = (& "C:\Program Files\GitHub CLI\gh.exe" auth token)
bun run --cwd shell build
```

Что делает:
1. `cargo build --release` для `kepler-backend.exe` + `ark-core-rpc.exe`.
2. `tsc && vite build` для renderer / main / preload.
3. `vite build` для каждого extension (`build:extensions`).
4. `electron-builder --win nsis` — NSIS installer.
5. **`electron-builder publish`** — push installer + `latest.yml` в release.

**Extensions НЕ bundled** в installer — это lean distribution. После
установки Kepler пустой, пользователь сам ставит расширения через
Marketplace или drag-and-drop `.kext` файлов.

## Kosmos extensions: marketplace

### catalog.json

Single source of truth для launcher'а — что доступно установить.
Auto-генерируется из GitHub Releases (`gh api releases`):
- Группировка по prefix tag'а (`horologion-`, `delphi-`, …).
- Per group выбирается **highest semver**.
- Metadata (name, description, keplerApiVersion) тянется из локального
  `extensions/<id>/manifest.json` в monorepo.

Schema:

```json
{
  "schemaVersion": 1,
  "updatedAt": "ISO timestamp",
  "extensions": [
    {
      "id": "horologion",
      "name": "Horologion",
      "description": "Трекер времени и pomodoro",
      "author": "yoso-industries",
      "version": "0.3.0",
      "keplerApiVersion": "^1.0.0",
      "iconUrl": "https://raw.githubusercontent.com/.../icon.png",
      "downloadUrl": "https://github.com/.../horologion-0.3.0.kext",
      "sha256": "abc...",
      "size": 12345
    }
  ]
}
```

Launcher tolerant к unknown fields. Breaking change → bump `schemaVersion` +
оставить backward read на 1 release.

### Marketplace UI

Settings → **Расширения** — плоский список установленных, каталог используется только для lookup версий (без отдельного «Маркетплейс» sub-tab'а и без grid карточек):
- На mount таб fetch'ит `catalog.json` через
  `window.kepler.extension.catalogFetch()`.
- Cache 1h в main process (`extension-marketplace.ts → fetchCatalog`).
- Per-row: сравнение `catalog[id].version` с `installed.version` — если catalog свежее, появляется кнопка **«Обновить»**.
- Кнопка **«Проверить обновления»** в шапке таба зовёт `loadCatalog(force=true)` (минует cache).
- Для каждой установленной строки также доступны **«Откатить»** (если в `extensions-backups/<id>/` есть копия) и **«Удалить»**.
- Секций «Прочие установленные» / «Каталог пуст» / hint «Каталог обновлён: `<date>`» больше нет — каталог не показывается как самостоятельная витрина.

### Periodic check

`startPeriodicCatalogCheck()`:
- Initial fetch на `whenReady` (вместе с `setupAutoUpdater()`).
- `setInterval(24h)` для перефетча в фоне.
- Skip в `KOSMOS_TEST_MODE=1`.

### Install flow

`window.kepler.extension.installFromUrl(downloadUrl, sha256)`:
1. HTTPS GET → tmp файл.
2. SHA-256 validate (если передан, обычно из catalog).
3. Delegate в `installFromPath` из `extension-installer.ts` (Wave 1 infra:
   backup → atomic rename → revert ready).

## Команды

| Команда | Описание |
|---|---|
| `bun run --cwd shell build` | Полный build launcher'а + publish в kepler-releases (нужен `GH_TOKEN`) |
| `bun run --cwd shell ext:publish <id>` | Build extension → .kext → release в kosmos-extensions |
| `bun run --cwd shell ext:publish-all` | То же для всех extensions |
| `bun run --cwd shell ext:catalog -- <out-path>` | Регенерация catalog.json из GitHub releases |

`shell/scripts/publish-extension.mjs` использует `gh release create` —
автоматически берёт `gh auth token` если `KEPLER_GH_PATH` не указан.

`shell/scripts/generate-catalog.mjs` использует `gh api releases --paginate`.

### Submission flow

Сейчас (Phase 1):
1. Maintainer (`ksanrse`) пишет / правит extension в `extensions/<id>/`.
2. `bun run --cwd shell ext:publish <id>` → release в `kosmos-extensions`.
3. `bun run --cwd shell ext:catalog -- .tmp/kosmos-extensions/catalog.json`.
4. Commit + push catalog.json в `kosmos-extensions` main branch.

Phase 2 (когда появятся внешние contributors):
- Fork main `yoso-industries/kepler` monorepo.
- PR с extension'ом в `extensions/<id>/`.
- Maintainer review + merge + publish.

## Ограничения

- Windows-only (Phase 1). macOS / Linux installers — out of scope.
- Code signing skip пока v0.x — SmartScreen warning при первой установке
  Kepler. Cert ~$300/y когда стабильно.
- Один stable channel. Beta / dev channels — defer.
- Нет auto-rollback после bad release (Phase 2).
- Нет community PR submission flow пока (Phase 2).
- Нет GitHub Actions для auto-build на push — solo dev, всё локально.

## См. также

- [Extension installer](/concepts/extension-installer) — Wave 1 `.kext` infrastructure.
- [Extension host](/concepts/extension-host) — extension lifecycle в launcher'е.
- [Forbidden](/agents/forbidden) — список запретов distribution flow.
