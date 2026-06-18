# Distribution

::: tip TL;DR

- **Kosmos launcher** дистрибутится через `makekosmos/desktop` (NSIS/DMG/Linux installers + electron-updater metadata).
- **Расширения** дистрибутятся через `makekosmos/extensions` (per-extension tagged releases + `catalog.json`).
- **Source code** остаётся в `ksanrse/kepler` monorepo. Distribution repos — binary only.
  :::

## Двухрепозиторная Raycast-style модель

```
ksanrse/kepler (private monorepo)         ← source of truth
        │
        │  bun run --cwd platform/desktop build
        ↓                                  bun run --cwd platform/desktop ext:publish ID
makekosmos/desktop                          makekosmos/extensions
     latest.yml                               delphi-v0.2.0/
                                              ...
                                            catalog.json (auto-regenerated)
        │                                          ↑
        │  HTTPS (electron-updater)                │ HTTPS (catalog fetch)
        ↓                                          │
   End-user Kepler.exe ───────────────────────────┘
```

Launcher и marketplace полностью независимы: launcher обновляется через
`electron-updater`, а уже установленные user extensions обновляются фоном из
marketplace catalog.

## Kepler launcher: autoUpdater

Реализация — `platform/desktop/electron/autoupdater-host.ts` (state machine поверх
`electron-updater`). Вызывается из `platform/desktop/electron/main.ts` как
`setupAutoUpdater({ isDev })`.

- Skip в dev mode (`isDev: true`, передаётся из main.ts когда
  `VITE_DEV_SERVER_URL` set) и test mode (`KOSMOS_TEST_MODE=1`).
- `check()` на старте + каждые **6 часов** (`setInterval`).
- `autoDownload = true`, `autoInstallOnAppQuit = false` — установку драйвит
  пользователь кликом по banner'у.
- `update-downloaded` → broadcast `kind: "downloaded"`. Native dialog
  «Перезапустить сейчас?» появляется как **fallback через 5 минут**, если
  пользователь не нажал banner.

Конфиг publish'а — в `platform/desktop/package.json` как **per-platform** массивы
(`build.win.publish` / `build.mac.publish`). Верхнеуровневый `build.publish` удалён.

- Windows: `build.win.publish[0]` = `makekosmos/desktop` (единственный канал).
- Mac: `build.mac.publish[0]` = `makekosmos/desktop-mac`.

Mac migration bridge: существующие Mac установки имеют `app-update.yml` с
`makekosmos/desktop` (не `desktop-mac`). Чтобы мигрировать их на новый канал,
нужно опубликовать **один переходный Mac release в ОБА** репозитория —
`makekosmos/desktop` и `makekosmos/desktop-mac`. После этого окна все новые Mac установки будут смотреть только на
`makekosmos/desktop-mac`.

### State machine

```
idle → checking → (available → downloading → downloaded) | not-available | error
any → checking (manual or periodic)
```

`UpdateState` (см. `autoupdater-host.ts`):

| kind            | поля                                 |
| --------------- | ------------------------------------ |
| `idle`          | —                                    |
| `checking`      | —                                    |
| `not-available` | `checkedAt: number`                  |
| `available`     | `version: string`                    |
| `downloading`   | `version: string`, `percent: number` |
| `downloaded`    | `version: string`                    |
| `error`         | `message: string`                    |

Broadcast идёт на IPC канал **`kepler:settings:update:state`** для **всех**
`BrowserWindow`'ов (launcher / settings / extensions). UI слушает и
ререндерит banner.

### Public API (main process)

```ts
import { setupAutoUpdater, check, install, getState } from "./autoupdater-host";

setupAutoUpdater({ isDev }); // init + первый check + 6h interval
await check(); // manual force-check (idempotent)
install(); // quitAndInstall — для click-to-install
getState(); // текущий UpdateState (snapshot)
```

### IPC handlers / renderer API

| IPC channel                      | direction             | payload                    |
| -------------------------------- | --------------------- | -------------------------- |
| `kepler:settings:update:check`   | invoke                | → `UpdateState`            |
| `kepler:settings:update:install` | invoke                | → `void`                   |
| `kepler:settings:update:state`   | invoke                | → `UpdateState` (snapshot) |
| `kepler:settings:update:state`   | event (main→renderer) | `UpdateState` (push)       |

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

`platform/desktop/src/views/SettingsView.vue` рендерит sticky banner высотой 32px
сверху Settings окна, если `state.kind !== "idle"` и `!= "not-available"`.

| State         | Текст                                                        | Icon             | Поведение клика                                          |
| ------------- | ------------------------------------------------------------ | ---------------- | -------------------------------------------------------- |
| `available`   | «Обновление Kepler X.Y.Z — нажми чтобы скачать»              | `ArrowUpCircle`  | autoDownload уже идёт, click no-op (или повторный check) |
| `downloading` | progress bar + `{percent}%`                                  | `Loader2` (spin) | disabled                                                 |
| `downloaded`  | «Обновление готово — нажми чтобы установить и перезапустить» | `ArrowUpCircle`  | `install()` → quitAndInstall                             |
| `error`       | сообщение error'а                                            | —                | retry check                                              |

Иконки — `@lucide/vue`.

### Manual «Проверить обновления»

Доступно двумя способами:

1. **Settings → General → кнопка «Проверить обновления»** — вызывает
   `window.kepler.settings.update.check()`.
2. **Launcher команда `kepler:check-updates`** (Ctrl+Shift+K → «Проверить
   обновления»). Хендлер `runCheckUpdates()` в `platform/desktop/electron/commands.ts`
   только вызывает `check()` из autoupdater-host — окно настроек не
   открывается. Обновлённый state приходит в launcher через update banner
   (pinned tile в секции «Обновление») и параллельно в Settings, если оно
   уже открыто.

### Tray icon в production

В production окно tray грузит иконку из `process.resourcesPath/icon.png`.
`platform/desktop/electron/main.ts → createTray()` пробует candidate-paths по
приоритету:

1. `<process.resourcesPath>/icon.png` (production, кладётся через
   `extraResources` в `platform/desktop/package.json → build`).
2. `<__dirname>/../build/icon.png` (dev fallback).
3. `<__dirname>/../../build/icon.png` (dev из dist-electron).

Без этого в production install tray icon был чёрным placeholder'ом —
`__dirname` указывал на `<install>/resources/app.asar/dist-electron/` и
относительный путь не резолвился.

### Per-platform versioning

**Rationale**: `electron-updater`'s GitHubProvider resolves a single "latest release"
per repo — interleaving Windows-only and Mac-only releases in one repo breaks the
other platform's updater (it reads a `latest.yml` / `latest-mac.yml` that may not
be present in the "latest" release).

**Solution**: Windows and Mac publish to separate repos; each repo has its own
independent "latest release".

**Version scheme**:

- `MAJOR.MINOR` is a shared **feature-parity line** — bumped together when a feature
  ships on both platforms.
- `PATCH` is **independent per platform** — each platform ships its own bug-fix
  patches without waiting for the other.
- Example: Windows `0.5.32` and Mac `0.5.5` are both on the "0.5" parity line.

**Source of truth**: `platform/desktop/release-versions.json`

```json
{ "win": "0.5.3", "mac": "0.5.1" }
```

**Bump CLI** (`platform/desktop/scripts/release-version.mjs`):

| Command                                                        | Effect                                              |
| -------------------------------------------------------------- | --------------------------------------------------- |
| `node scripts/release-version.mjs get <win\|mac>`              | Print current version (used by build wrapper)       |
| `node scripts/release-version.mjs bump --platform win`         | Windows PATCH +1 (default)                          |
| `node scripts/release-version.mjs bump --platform mac --minor` | Mac MINOR +1, PATCH → 0                             |
| `node scripts/release-version.mjs bump --minor`                | Both platforms MINOR +1, PATCH → 0 (parity release) |
| `node scripts/release-version.mjs bump --major`                | Both platforms MAJOR +1, MINOR 0, PATCH 0           |

Default bump = PATCH. `--minor` / `--major` must be explicit.

### Релиз launcher'а

```powershell
# Один раз: $env:GH_TOKEN = (& "C:\Program Files\GitHub CLI\gh.exe" auth token)

# Windows release:
bun run --cwd platform/desktop build

# Mac release:
bun run --cwd platform/desktop build:mac
```

Что делает `bun run build` (Windows):

1. `cargo build --release` для `kepler-backend.exe` + `ark-core-rpc.exe`.
2. `tsc && vite build` для renderer / main / preload.
3. `vite build` для каждого extension (`build:extensions`).
4. `node scripts/build-desktop.mjs --platform win`:
   - Читает версию из `release-versions.json["win"]`.
   - `electron-builder --win nsis --publish always -c.extraMetadata.version=<v>` — NSIS installer + `latest.yml` в `makekosmos/desktop`.
   - После 0-exit: `node scripts/verify-release-channel.mjs --platform win --version <v>` — проверяет целостность опубликованного релиза.

Mac (`bun run build:mac`) аналогично, но `electron-builder --mac dmg --publish always` и publish в `makekosmos/desktop-mac`, verify на `latest-mac.yml`.

**Extensions НЕ bundled** в installer — это lean distribution. После
установки Kepler пустой, пользователь сам ставит расширения через
Marketplace или drag-and-drop `.kext` файлов.

## Kosmos extensions: marketplace

### catalog.json

Single source of truth для launcher'а — что доступно установить.
Auto-генерируется из GitHub Releases (`gh api releases`):

- Per group выбирается **highest semver**.
- Metadata (name, description, keplerApiVersion) тянется из локального
  `extensions/ID/manifest.json` в monorepo.

Schema:

```json
{
  "schemaVersion": 1,
  "updatedAt": "ISO timestamp",
  "extensions": [
    {
      "description": "Трекер времени и pomodoro",
      "author": "makekosmos",
      "version": "0.3.0",
      "keplerApiVersion": "^1.0.0",
      "iconUrl": "https://raw.githubusercontent.com/.../icon.png",
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
- Для каждой установленной строки также доступны **«Откатить»** (если в `extensions-backups/ID/` есть копия) и **«Удалить»**.
- Секций «Прочие установленные» / «Каталог пуст» / hint «Каталог обновлён: `DATE`» больше нет — каталог не показывается как самостоятельная витрина.

### Periodic check

`startPeriodicCatalogCheck()`:

- Initial fetch на `whenReady` (вместе с `setupAutoUpdater()`) и unattended
  update уже установленных user extensions.
- `setInterval(24h)` для force-перефетча и unattended update в фоне.
- Skip в `KOSMOS_TEST_MODE=1`.
- Skip в dev / `dev-X` slots через `periodicMarketplaceCheckEnabled=false`.
- Обновляются только `source: "installed"` extension'ы. Repo dev-source
  extension'ы не трогаются.
- Новые extension'ы из каталога автоматически не ставятся.
- Решение об update — strict semver: catalog version должна быть новее
  installed version; равные/старые/невалидные версии пропускаются.
- Каждый update идёт через `installFromUrl(downloadUrl, sha256)` →
  `installFromPath`, то есть сохраняет SHA-256 validation, backup и atomic
  install. Ошибка одного extension'а логируется и не останавливает остальные.
- Если Vue-extension уже открыт, его окно reload'ится после успешного install,
  чтобы пользователь не ждал ручного переоткрытия.

### Install flow

`window.kepler.extension.installFromUrl(downloadUrl, sha256)`:

1. HTTPS GET → tmp файл.
2. SHA-256 validate (если передан, обычно из catalog).
3. Delegate в `installFromPath` из `extension-installer.ts` (Wave 1 infra:
   backup → atomic rename → revert ready).

## Команды

| Команда                                                                       | Описание                                                                            |
| ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `bun run --cwd platform/desktop build`                                        | Полный build Windows launcher'а + publish в `makekosmos/desktop` (нужен `GH_TOKEN`) |
| `bun run --cwd platform/desktop build:mac`                                    | Полный build Mac launcher'а + publish в `makekosmos/desktop-mac` (нужен `GH_TOKEN`) |
| `node scripts/release-version.mjs bump --platform win`                        | Windows PATCH +1 (patch default)                                                    |
| `node scripts/release-version.mjs bump --platform mac --minor`                | Mac MINOR +1, PATCH → 0                                                             |
| `node scripts/release-version.mjs bump --minor`                               | Оба платформы MINOR +1 (parity release)                                             |
| `bun run --cwd platform/desktop verify:channel -- --platform win --version X` | Проверить целостность Windows release                                               |
| `bun run --cwd platform/desktop verify:channel -- --platform mac --version X` | Проверить целостность Mac release                                                   |
| `bun run --cwd platform/desktop ext:publish ID`                               | Build extension → .kext → release в makekosmos/extensions                           |
| `bun run --cwd platform/desktop ext:publish-all`                              | То же для всех extensions                                                           |
| `bun run --cwd platform/desktop ext:catalog -- OUT_PATH`                      | Регенерация catalog.json из GitHub releases                                         |

`platform/desktop/scripts/publish-extension.mjs` использует `gh release create` —
автоматически берёт `gh auth token` если `KEPLER_GH_PATH` не указан.

`platform/desktop/scripts/generate-catalog.mjs` использует `gh api releases --paginate`.

### Submission flow

Сейчас (Phase 1):

1. Maintainer (`ksanrse`) пишет / правит extension в `extensions/ID/`.
2. `bun run --cwd platform/desktop ext:publish ID` → release в `makekosmos/extensions`.
3. `bun run --cwd platform/desktop ext:catalog -- .tmp/extensions/catalog.json`.
4. Commit + push catalog.json в `makekosmos/extensions` main branch.

Phase 2 (когда появятся внешние contributors):

- Fork main `ksanrse/kosmos` monorepo.
- PR с extension'ом в `extensions/ID/`.
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
