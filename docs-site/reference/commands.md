# Команды и скрипты

Шпаргалка по всем командам, которые есть в репо.

::: tip Конвенция сборки релизов
Для всех desktop-приложений Kosmos `bun run build` производит **финальный установщик NSIS one-click** (тот самый «плавный» опыт как у Linear / Slack / Discord / GitHub Desktop):

```json
"nsis": {
  "oneClick": true,
  "perMachine": false,
  "allowToChangeInstallationDirectory": false,
  "createDesktopShortcut": true,
  "createStartMenuShortcut": true,
  "shortcutName": "<AppName>",
  "runAfterFinish": true,
  "deleteAppDataOnUninstall": false
}
```

Почему не MSI: MSI — корпоративный формат, диктует step-by-step wizard, требует admin-прав, ставит в `Program Files`, не стилизуется. Все современные Electron-приложения используют NSIS one-click или Squirrel.

`perMachine: false` + `oneClick: true` означают установку в `%LocalAppData%\<AppName>` без UAC-промпта и без мастера — пользователь дважды кликает по `Setup.exe`, видит короткий прогресс, приложение запускается (`runAfterFinish: true`).

Промежуточная сборка только JS/Rust артефактов (без установщика) — `bun run build:js` или `bun run package:dir` для unpacked-бандла.
:::

::: warning Bump версии после билда
**После каждого успешного `bun run build`** (который произвёл установщик NSIS) — поднимаем `version` приложения в его `package.json` на `+0.0.1` (patch). Делается **сразу же**, в том же коммите, что и сам билд, чтобы следующий релиз не перезаписал предыдущий installer-файл (`release/<App> Setup X.Y.Z.exe`).

Минорные/мажорные bump'ы (`+0.1.0` / `+1.0.0`) — только по явному решению человека (новая большая фича, breaking change). По умолчанию — patch.
:::

## Корневые

Из корня `kepler/`:

```powershell
bun install                      # установка зависимостей всех workspaces

bun run ark:guard:writes         # запрет прямых SQL writes в ARK SQLite из app services
                                  # → scripts/check-ark-write-boundaries.mjs

bun run ark:smoke                # ARK smoke matrix
                                  # → scripts/ark-smoke.mjs

bun run docs:dev                 # запустить этот сайт (http://localhost:5173)
bun run docs:sync                # регенерация AGENTS.md / CLAUDE.md / llms.txt
bun run docs:check               # верификация stale references в docs-site/
bun run docs:build               # docs:sync + статическая сборка
bun run docs:preview             # превью собранного
```

## ARK runtime

```powershell
cargo build --workspace                                                # всё через Cargo workspace
cargo build --manifest-path crates\ark-core\rust\Cargo.toml --bin ark-core-rpc
cargo test  --manifest-path crates\ark-core\rust\Cargo.toml

bun run --cwd packages/ark typecheck
bun run --cwd packages/ark build
bun run --cwd packages/ark test
```

## Kepler Shell (главный путь)

`shell/` — Electron-лаунчер Kepler (фронт для экосистемы Kosmos). Окно fixed-size 720×460, command bus как primary integration primitive.

```powershell
cd shell
bun run dev                # backend + extensions + Vite + Electron
bun run typecheck          # TS check (renderer + main + preload)
bun run build:js           # сборка renderer + main + preload + extensions (без установщика)
bun run build              # full production chain:
                            #   1. cargo build --release services/kepler-backend
                            #   2. tsc + vite (renderer / main / preload)
                            #   3. vite build per extension × 4 (Dashboard / Horologion / Delphi / Arrancador)
                            #   4. electron-builder --win nsis (one-click installer)
bun run package:dir        # unpacked desktop bundle
bun run test:e2e           # Playwright smoke
bun run ext:install <path> # положить extension override в %APPDATA%\Kosmos\extensions\
bun run ext:uninstall <id>
```

Output финального билда:

- Installer: `shell/release/Kepler Setup X.Y.Z.exe` (per-user oneClick).
- Install path: `%LOCALAPPDATA%\Programs\Kepler\` (без UAC, без выбора директории).
- Launch: `runAfterFinish: true`, ярлык на рабочем столе + Start Menu.

Extension dev mode (Raycast-style HMR, см. [Extension dev mode](/concepts/extension-dev-mode)):

```powershell
# Vite dev servers per extension (порты 5180–5183)
bun run --cwd shell dev:extensions

# Kepler shell с включённым dev режимом (loadURL вместо loadFile для extensions)
$env:KEPLER_DEV = "1"; bun run --cwd shell dev

# Авто-открыть все 4 extension'а через 5s после старта (для RAM benchmark или smoke)
$env:KEPLER_BENCHMARK_OPEN_ALL = "1"; bun run --cwd shell dev
```

Extension bundles лежат в `extensions/<id>/` (Dashboard / Horologion / Delphi / Arrancador). Eden — outlier, остаётся standalone .exe (до Phase 6).

## Kepler Backend (Rust)

`services/kepler-backend/` — supervisor для `ark-core-rpc`, WS-gateway, command bus, sync, встроенный usage_tracker модуль.

```powershell
cargo build --manifest-path services/kepler-backend/Cargo.toml --bin kepler-backend
cargo test  --manifest-path services/kepler-backend/Cargo.toml --lib
```

## Eden (standalone)

```powershell
cd apps/eden/ts
bun run dev                # сборка Heart sidecar + Vite + Electron
bun run build              # production NSIS installer
bun run lint               # oxlint
bun run format             # oxfmt --check
bun run test:e2e           # build + Playwright
bun run test:ark-migration # smoke для ARK миграции
```

## Vue-extensions

Сборка extension'ов проходит **через Kepler shell**. У каждого extension'а есть свой `vite.config.mjs`, но командой `bun run --cwd shell build:extensions` Kepler shell их все билдит подряд.

```powershell
bun run --cwd shell build:extensions     # билд всех extensions/<id>/dist
bun run --cwd shell dev:extensions       # HMR dev servers на портах 5180-5183
```

## Android

### ark-service (Room ContentProvider)

```powershell
cd mobile/ark-service
.\gradlew build
```

### Delphi Android (UI)

```powershell
cd mobile/delphi
.\gradlew build
```

Для запуска на устройстве — обе APK должны быть установлены и подписаны одним ключом (signature permission `com.kosmos.ark.data.READ_WRITE`).

## Smoke матрица (по разделу)

```powershell
# Зафиксировать корень
$env:KOSMOS_SMOKE_ROOT = ".agent\tasks\<TASK>\smoke"
New-Item -ItemType Directory -Force -Path $env:KOSMOS_SMOKE_ROOT | Out-Null

# Core
cargo test --manifest-path crates\ark-core\rust\Cargo.toml
cargo build --manifest-path crates\ark-core\rust\Cargo.toml --bin ark-core-rpc

# Kepler backend (включая usage_tracker модуль)
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib

# SDK
bun run --cwd packages/ark typecheck
bun run --cwd packages/ark test

# Eden
bun run --cwd apps/eden/ts test:ark-migration

# Guard
bun run ark:guard:writes
```

См. [Smoke-матрица](/reference/smoke-matrix).

## Migration / служебные скрипты

После brand swap Kepler ↔ Kosmos (2026-05-14) появился набор служебных PowerShell/JS-скриптов:

```powershell
# Перенос всех Kepler-имён в Kosmos-имена (и обратные точки) в репозитории.
pwsh scripts/migrate-kepler-to-kosmos.ps1

# Проверка completeness swap: ищет остаточные «Kepler»/«Kosmos» паттерны,
# где их быть не должно. Падает при несоответствии — гард для PR.
pwsh scripts/check-swap-completeness.ps1

# Замер RAM Kepler shell vs standalone-апок. См. /concepts/ram-benchmarks.
pwsh scripts/measure-kepler-ram.ps1 -Mode kepler
pwsh scripts/measure-kepler-ram.ps1 -Mode baseline

# Чинит mojibake (UTF-8 vs CP1251) в .md / .ts / .vue, накопленный при правках
# в смешанной кодировке.
node scripts/fix-mojibake.mjs
```
