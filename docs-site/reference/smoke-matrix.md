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
cargo test  --manifest-path crates\ark-core\rust\Cargo.toml
cargo build --manifest-path crates\ark-core\rust\Cargo.toml --bin ark-core-rpc
bun run --cwd core/ark/packages/ark typecheck
bun run --cwd core/ark/packages/ark test
```

## Kepler Backend (включая usage_tracker модуль)

```powershell
cargo build --manifest-path services\kepler-backend\Cargo.toml --bin kepler-backend
cargo test  --manifest-path services\kepler-backend\Cargo.toml --lib
```

## Kepler Shell (launcher)

```powershell
bun run --cwd platform/desktop typecheck
bun run --cwd platform/desktop build:js
bun run --cwd platform/desktop test:e2e
```

`test:e2e` — Playwright smoke по лаунчеру (открытие окна 720×460, выполнение зарегистрированной команды через command bus).

## Eden extension

```powershell
bun run --cwd platform/desktop build:extensions
bun run --cwd platform/desktop typecheck
# Eden-specific e2e (Phase 6.0.5+): tests/e2e/eden.spec.ts
cd platform/desktop; bunx playwright test --config playwright.config.ts --grep "eden"
```

Phase 6.0.A удалил standalone Eden; ARK migration smoke больше не нужен — все writes идут через `kepler-api-shim` поверх ARK (которые покрыты `ark:guard:writes` + cargo tests).

::: danger
Не запускай migration / backfill скрипты против реальной ARK DB во время smoke verification. Передавай явные source и target пути под smoke root, когда нужна ручная migration check.
:::

## Guard перед PR в data services

```powershell
bun run ark:guard:writes
```

Запрещает прямые SQL writes в ARK SQLite из app services / extension'ов. Должен пройти зелёным.

## Что НЕ входит в smoke (на 2026-05-14)

После Phase B-D из smoke выпали:

- Dashboard `seedSmokeDb.ts` / `smokeAnalytics.ts` — переехали в архив, к smoke не подключены до Phase 6.
- Standalone `usage-tracker.exe` — заморожен в `legacy/usage-tracker/`. Активные тесты — внутри `platform/runtime` lib.

## Связанные документы

- [Изоляция тестовых БД](/concepts/test-isolation).
- [Граница записи в ARK](/concepts/write-boundary).
- [Команды и скрипты](/reference/commands).
