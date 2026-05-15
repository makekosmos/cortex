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

`shell/electron/main.ts → setupAutoUpdater()`:
- Skip в dev mode (`VITE_DEV_SERVER_URL` set) и test mode (`KOSMOS_TEST_MODE=1`).
- `checkForUpdatesAndNotify()` на старте + каждые **6 часов** (`setInterval`).
- `update-downloaded` → native dialog «Перезапустить сейчас?» → `quitAndInstall()` если yes.
- Logging всех событий в console (`update-available`, `download-progress`, `error`).

Конфиг publish'а — в `shell/package.json → build.publish[0]` (provider github,
owner yoso-industries, repo kepler-releases).

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

Settings → Extensions → **Маркетплейс** (sub-tab):
- На mount fetch'ит `catalog.json` через
  `window.kepler.extension.catalogFetch()`.
- Cache 1h в main process (`extension-marketplace.ts → fetchCatalog`).
- Per-card state computed sравнением `catalog[i].version` с
  `installedById(id).version`: «Установить» / «Обновить» / «Установлено».
- Badge с числом updates available — `computed` от catalog + installed.

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
