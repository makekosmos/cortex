# Smoke-матрица ARK

::: tip Источник правды
`docs/ARK-SMOKE-MATRIX.md`
:::

Матрица для интеграционных проверок ARK. Каждая команда обязана работать против **изолированной** тестовой БД (`.tmp`, `.e2e`, `.agent/tasks/<TASK_ID>/`, OS temp). **Никогда** не использовать main/user ARK DB для автоматических проверок.

## Корневая команда

```powershell
bun run ark:smoke
```

Перед task-локальными прогонами зафиксируй корень:

```powershell
$env:KOSMOS_SMOKE_ROOT = ".agent\tasks\<TASK>\smoke"
New-Item -ItemType Directory -Force -Path $env:KOSMOS_SMOKE_ROOT | Out-Null
```

## Core

```powershell
cargo test  --manifest-path packages\ark-core\rust\Cargo.toml
cargo build --manifest-path packages\ark-core\rust\Cargo.toml --bin ark-core-rpc
bun run --cwd packages/kosmos-ark typecheck
```

## Kepler Backend

```powershell
cargo build --manifest-path services\kepler-backend\Cargo.toml --bin kepler-backend
cargo test  --manifest-path services\kepler-backend\Cargo.toml --lib
```

## Kepler Shell (launcher)

```powershell
bun run --cwd apps/kepler-shell typecheck
bun run --cwd apps/kepler-shell build:js
bun run --cwd apps/kepler-shell test:e2e
```

`test:e2e` — Playwright smoke по лаунчеру (открытие окна 720×460, выполнение зарегистрированной команды через command bus).

## Usage tracker

```powershell
cargo test --manifest-path services\usage-tracker\Cargo.toml
```

Для ручных smoke прогонов передавай явный path:

```powershell
$env:ARK_DB_PATH = "$env:KOSMOS_SMOKE_ROOT\usage-tracker\ark.db"
```

## Dashboard

```powershell
node --experimental-strip-types apps\dashboard\scripts\seedSmokeDb.ts `
  --db-path "$env:KOSMOS_SMOKE_ROOT\dashboard\smoke-dashboard.db"

node --experimental-strip-types apps\dashboard\scripts\smokeAnalytics.ts `
  --db-path "$env:KOSMOS_SMOKE_ROOT\dashboard\smoke-dashboard.db"

bun run --cwd apps/dashboard test:e2e:smoke
```

## Delphi / Eden shared objects

```powershell
bun run --cwd apps/delphi/ts test:e2e -- e2e/shared-ark-task.spec.ts
bun run --cwd apps/eden/ts test:ark-migration
```

Эти проверки создают свои собственные временные app-data / vault paths и обязаны и дальше избегать main user databases.

## Arrancador

```powershell
bun run --cwd apps/arrancador test
bun run --cwd apps/arrancador smoke:packaged
```

`smoke:packaged` собирает unpacked Windows-бандл со включённым signing/editing-disabled специально для smoke, запускает `release/win-unpacked/arrancador.exe` и передаёт временные `APPDATA`, `LOCALAPPDATA`, `ARK_DB_PATH` под `apps/arrancador/.e2e/packaged-smoke`.

::: danger
Не запускай migration / backfill скрипты против реальной ARK DB во время smoke verification. Передавай явные source и target пути под smoke root, когда нужна ручная migration check.
:::

## Guard перед PR в data services

```powershell
bun run ark:guard:writes
```

Запрещает прямые SQL writes в ARK SQLite из app services. Должен пройти зелёным.

## Связанные документы

- [Изоляция тестовых БД](/concepts/test-isolation).
- [Граница записи в ARK](/concepts/write-boundary).
- [Команды и скрипты](/reference/commands).
