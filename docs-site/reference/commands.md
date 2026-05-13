# Команды и скрипты

Шпаргалка по всем командам, которые есть в репо.

::: tip Конвенция сборки релизов
Для всех desktop-приложений Kepler `bun run build` из директории приложения
производит **финальный установщик в формате MSI** (Windows Installer).
Это единый формат дистрибуции — установка per-machine, поддерживает enterprise-деплой,
unattended install и GPO. NSIS (`.exe`) больше не используем.

Промежуточная сборка только JS/Rust артефактов (без установщика) —
`bun run build:js` (если приложение её предоставляет) или `bun run package:dir`
для unpacked-бандла.
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
bun run docs:build               # собрать сайт в docs-site/.vitepress/dist
bun run docs:preview             # превью собранного
```

## ARK runtime

```powershell
cargo build --manifest-path packages\ark-core\rust\Cargo.toml --bin ark-core-rpc
cargo test  --manifest-path packages\ark-core\rust\Cargo.toml

bun run --cwd packages/kepler-ark typecheck
bun run --cwd packages/kepler-ark build
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
bun run build            # build:js + electron-builder --win msi (финальный MSI)
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

## Horologion

Workspace-директория исторически осталась `apps/horologion`, имя приложения — Horologion.

```powershell
cd apps/horologion
bun run dev              # build:sidecar:dev + Vite + Electron
bun run build:js         # sidecar release + TS + Vite (без установщика)
bun run build            # build:js + electron-builder --win msi (финальный MSI)
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
.\dist\KeplerUsageTrackerInstaller\install.ps1
.\dist\KeplerUsageTrackerInstaller\uninstall.ps1
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

Для запуска на устройстве — обе APK должны быть установлены и подписаны одним ключом (signature permission `com.kepler.ark.data.READ_WRITE`).

## Smoke матрица (по разделу)

```powershell
# Зафиксировать корень
$env:KEPLER_SMOKE_ROOT = ".agent\tasks\<TASK>\smoke"
New-Item -ItemType Directory -Force -Path $env:KEPLER_SMOKE_ROOT | Out-Null

# Core
cargo test --manifest-path packages\ark-core\rust\Cargo.toml
cargo build --manifest-path packages\ark-core\rust\Cargo.toml --bin ark-core-rpc
bun run --cwd packages/kepler-ark typecheck

# Usage tracker
cargo test --manifest-path services\usage-tracker\Cargo.toml

# Dashboard
node --experimental-strip-types apps\dashboard\scripts\seedSmokeDb.ts --db-path "$env:KEPLER_SMOKE_ROOT\dashboard\smoke-dashboard.db"
node --experimental-strip-types apps\dashboard\scripts\smokeAnalytics.ts --db-path "$env:KEPLER_SMOKE_ROOT\dashboard\smoke-dashboard.db"
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
