# Команды и скрипты

Шпаргалка по всем командам, которые есть в репо.

::: tip Конвенция сборки релизов
Для всех desktop-приложений Kosmos `bun run build` из директории приложения
производит **финальный установщик NSIS one-click** (тот самый «плавный»
опыт как у Linear / Slack / Discord / GitHub Desktop):

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

Почему не MSI: MSI — корпоративный формат, диктует step-by-step wizard,
требует admin-прав, ставит в `Program Files`, не стилизуется. Все
современные Electron-приложения используют NSIS one-click или Squirrel.

`perMachine: false` + `oneClick: true` означают установку в
`%LocalAppData%\<AppName>` без UAC-промпта и без мастера — пользователь
дважды кликает по `Setup.exe`, видит короткий прогресс, приложение
запускается (`runAfterFinish: true`).

Промежуточная сборка только JS/Rust артефактов (без установщика) —
`bun run build:js` (если приложение её предоставляет) или `bun run package:dir`
для unpacked-бандла.
:::

::: warning Bump версии после билда
**После каждого успешного `bun run build`** (который произвёл установщик
NSIS) — поднимаем `version` приложения в его `apps/<name>/package.json` на
`+0.0.1` (patch). Делается **сразу же**, в том же коммите, что и сам билд,
чтобы следующий релиз не перезаписал предыдущий installer-файл
(`release/<App> Setup X.Y.Z.exe`) и чтобы auto-update / changelog имел
монотонную последовательность.

Минорные/мажорные bump'ы (`+0.1.0` / `+1.0.0`) — только по явному решению
человека (новая большая фича, breaking change). По умолчанию — patch.
:::

## Корневые

Из корня `kosmos/`:

```powershell
bun install                      # установка зависимостей всех workspaces

bun run ark:guard:writes         # запрет прямых SQL writes в ARK SQLite из app services
                                  # → scripts/check-ark-write-boundaries.mjs

bun run ark:smoke                # ARK smoke matrix
                                  # → scripts/ark-smoke.mjs

bun run docs:dev                 # запустить этот сайт (http://localhost:5173)
bun run docs:build               # собрать сайт в docs-site/.vitepress/dist
bun run docs:preview             # превью собранного
```

## ARK runtime

```powershell
cargo build --manifest-path packages\ark-core\rust\Cargo.toml --bin ark-core-rpc
cargo test  --manifest-path packages\ark-core\rust\Cargo.toml

bun run --cwd packages/kosmos-ark typecheck
bun run --cwd packages/kosmos-ark build
```

## Eden

```powershell
cd apps/eden/ts
bun run dev               # сборка Rust + Vite + Electron
bun run build             # production (Rust release + TS + Vite)
bun run lint              # oxlint
bun run format            # oxfmt --check
bun run test:e2e          # build + Playwright
bun run test:ark-migration  # smoke для ARK миграции
bun run package           # DMG/installer
```

## Delphi

```powershell
cd apps/delphi/ts
bun run build:ark:dev    # debug ark-core-rpc
bun run build:ark        # release ark-core-rpc
bun run dev              # build:ark:dev + Vite + Electron
bun run build:js         # ark release + TS + Vite (без установщика)
bun run build            # build:js + electron-builder --win nsis (финальный NSIS one-click)
bun run package:dir      # unpacked desktop bundle (без установщика)
bun run test             # unit
bun run e2e              # Playwright (включая shared-ark-task.spec.ts)
```

## Arrancador

```powershell
cd apps/arrancador
bun run typecheck
bun run build:renderer
bun run build:main
bun run build:preload
bun run test
bun run smoke:packaged   # smoke на packaged Electron сборке (с временным APPDATA)
```

## Dashboard

```powershell
cd apps/dashboard
bun run build            # renderer + Electron bundles
bun run package:dir      # unpacked desktop bundle
bun run dist             # Windows NSIS
bun run smoke:seed       # сидинг smoke БД
bun run smoke:analytics  # CLI assertion аналитики
bun run test:e2e         # Playwright (с Python-сидингом в globalSetup)
bun run test:e2e:smoke   # прямой Playwright-library smoke
```

## Kepler Shell (launcher)

`apps/kepler-shell/` — Electron-лаунчер Kepler (фронт для экосистемы Kosmos). Окно fixed-size 720×460, command bus как primary integration primitive.

```powershell
cd apps/kepler-shell
bun run dev              # dev mode (Vite renderer + Electron main)
bun run typecheck        # TS check (renderer + main + preload)
bun run build:js         # сборка renderer + main + preload (без установщика)
bun run build            # build:js + electron-builder --win nsis (финальный NSIS one-click)
bun run test:e2e         # Playwright smoke
```

Extension dev mode (Raycast-style HMR, см. [Extension dev mode](/concepts/extension-dev-mode)):

```powershell
# Vite dev servers per extension (порты 5180–5183)
bun run --cwd apps/kepler-shell dev:extensions

# Kepler shell с включённым dev режимом (loadURL вместо loadFile для extensions)
$env:KEPLER_DEV = "1"; bun run --cwd apps/kepler-shell dev

# Авто-открыть все 4 extension'а через 5s после старта (для RAM benchmark или smoke)
$env:KEPLER_BENCHMARK_OPEN_ALL = "1"; bun run --cwd apps/kepler-shell dev
```

Extension bundles лежат в `apps/kepler-shell/extensions/<id>/` (Dashboard / Horologion / Delphi / Arrancador). Eden — outlier, остаётся standalone .exe.

## Kepler Backend (Rust)

`services/kepler-backend/` — Rust-сервис: command bus host, WS server для лаунчера и приложений.

```powershell
cargo build --manifest-path services/kepler-backend/Cargo.toml --bin kepler-backend
cargo test  --manifest-path services/kepler-backend/Cargo.toml --lib
```

## Horologion

Workspace-директория исторически осталась `apps/horologion`, имя приложения — Horologion.

```powershell
cd apps/horologion
bun run dev              # build:sidecar:dev + Vite + Electron
bun run build:js         # sidecar release + TS + Vite (без установщика)
bun run build            # build:js + electron-builder --win nsis (финальный NSIS one-click)
bun run package:dir      # unpacked desktop bundle (без установщика)
bun run typecheck
bun run test:e2e         # build:js + Playwright
```

## Usage tracker

```powershell
cd services/usage-tracker
cargo test --manifest-path Cargo.toml
bun run build:release           # release exe
bun run package:installer       # installer bundle + zip
```

Запуск с overrides:

```powershell
$env:ARK_DB_PATH = "C:\path\to\test\ark.db"
$env:USAGE_TRACKER_POLL_MS = "1000"
.\target\release\usage-tracker.exe --once
```

Установка / удаление:

```powershell
.\dist\KosmosUsageTrackerInstaller\install.ps1
.\dist\KosmosUsageTrackerInstaller\uninstall.ps1
```

## Android

### ark-service (Room ContentProvider)

```powershell
cd apps/ark-service
.\gradlew build
```

### Delphi Android (UI)

```powershell
cd apps/delphi/kotlin
.\gradlew build
```

Для запуска на устройстве — обе APK должны быть установлены и подписаны одним ключом (signature permission `com.kosmos.ark.data.READ_WRITE`).

## Smoke матрица (по разделу)

```powershell
# Зафиксировать корень
$env:KOSMOS_SMOKE_ROOT = ".agent\tasks\<TASK>\smoke"
New-Item -ItemType Directory -Force -Path $env:KOSMOS_SMOKE_ROOT | Out-Null

# Core
cargo test --manifest-path packages\ark-core\rust\Cargo.toml
cargo build --manifest-path packages\ark-core\rust\Cargo.toml --bin ark-core-rpc
bun run --cwd packages/kosmos-ark typecheck

# Usage tracker
cargo test --manifest-path services\usage-tracker\Cargo.toml

# Dashboard
node --experimental-strip-types apps\dashboard\scripts\seedSmokeDb.ts --db-path "$env:KOSMOS_SMOKE_ROOT\dashboard\smoke-dashboard.db"
node --experimental-strip-types apps\dashboard\scripts\smokeAnalytics.ts --db-path "$env:KOSMOS_SMOKE_ROOT\dashboard\smoke-dashboard.db"
bun run --cwd apps/dashboard test:e2e:smoke

# Delphi / Eden shared objects
bun run --cwd apps/delphi/ts test:e2e -- e2e/shared-ark-task.spec.ts
bun run --cwd apps/eden/ts test:ark-migration

# Arrancador
bun run --cwd apps/arrancador test
bun run --cwd apps/arrancador smoke:packaged

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

