# Правила репозитория

Сжатый чек-лист правил. Полные тексты — в `docs/`, `AGENTS.md` каждого приложения и в [концептах](/concepts/architecture).

## 1. Граница записи в ARK

::: danger
- Все ARK writes через `@kosmos/ark` (TS) или `ark_core::db` (Rust).
- **Прямые SQL writes** в `objects` / `object_types` / `object_links` / `tracked_apps` / `usage_sessions` / `usage_events` / `sync_kv` из app services — **запрещены**.
- Dashboard — read-only.
- Перед PR в data services: `bun run ark:guard:writes`.
:::

См. [Граница записи в ARK](/concepts/write-boundary).

## 2. Read-only SQL

- Renderer **никогда** не открывает SQLite напрямую.
- В Electron main read-only SQLite — fallback, отделённый от write paths.
- Dashboard — единственный полноценный read-only inspector.
- Read-only пути **не запускаются** против user DB в автотестах.

См. [Read-only SQL boundary](/concepts/readonly-sql).

## 3. Изоляция тестовых БД

::: danger
- Тесты / smoke / Playwright / migration verify — **только** на изолированных DB.
- Разрешённые пути: `.tmp`, `.e2e`, `.agent/tasks/<TASK_ID>/smoke/`, OS temp.
- Любой тест, дефолтящийся в user data dir, **отвергается** на code review.
:::

См. [Изоляция тестовых БД](/concepts/test-isolation).

## 4. Proof loop

Substantial-правки идут через `.agent/tasks/<DATE>-<slug>/`:

`spec.md` → реализация → `evidence.{md,json}` → если не PASS → `problems.md` → fix → reverify.

Каждый AC должен быть `PASS`. См. [Proof loop](/concepts/proof-loop).

## 5. Sync state у direct writers

- Любой Rust-writer, пишущий напрямую в синхронизируемые таблицы, **обязан** вызывать `ark_core::db::bump_sync_version_vector` (или хелпер, который это делает).
- Self-peer filtering и routable-address filtering — обязательные инварианты sync. Не ослабляй.

См. [Синхронизация](/concepts/sync).

## 6. Tooling-минимум

- `bun install` после клона.
- `bun run ark:guard:writes` перед PR в data-слой.
- `bun run ark:smoke` перед нетривиальным PR.
- `cargo test` в `crates/ark-core/rust` при правках runtime.

См. [Стек и инструменты](/guide/tooling) и [Smoke-матрица](/reference/smoke-matrix).

## 7. UI и Visuals

- Используй `@kosmos/visuals` для shared chrome / сайдбара / titlebar.
- **Не копируй** shared компоненты внутрь приложения.
- Не возвращай ручные titlebar-offset / safe-area хаки — есть `DesktopChrome` / `DesktopContentSurface`.

См. [kosmos-visuals](/packages/visuals).

## 8. Запреты per-app

| Приложение | Не делать |
|---|---|
| Delphi | Восстанавливать legacy DB sidecar / использовать old todo таблицы как long-term fallback |
| Eden | Возвращаться к ripgrep, ломать `save/move/delete` hardening в `store.ts`, возвращать ручные titlebar-offset |
| Arrancador | Возвращать собственный usage tracker / window polling, добавлять Tauri или React пути |
| Dashboard | Открывать SQLite в renderer, дублировать ARK queries вне `electron/services/analytics.ts` |

## 9. Brand consistency (Kepler / Kosmos)

После swap 2026-05-14:

- **Kepler** — имя лаунчера и его UI-shell. `shell/`, `services/kepler-backend/`, `measure-kepler-ram.ps1` и т.п.
- **Kosmos** — имя экосистемы / монорепо / shared packages. `@kosmos/ark`, `@kosmos/visuals`, ARK runtime, doc-site, общий бренд.
- Не смешивай: «Kosmos launcher» — неверно, это **Kepler**. «Kepler ARK» — неверно, ARK живёт в **Kosmos**.
- Перед PR прогоняй `pwsh scripts/check-swap-completeness.ps1` если правил что-то рядом с брендом.

## 10. Command bus

- Apps регистрируют свои commands через `ArkClient.commands.register(...)` **только** в `kepler-mode` (когда лаунчер их вызвал). Регистрация — в `try/catch`: standalone-режим (без лаунчера) не имеет commands API, и это норма, не ошибка.
- Wire format событий command bus — **flat**: `{event: "command:invoked", id: "...", ...fields}`. Не `{kind: "event", type: "...", payload: {...}}`. Согласовано с peer/sync events.
- Command-категории в `shell/electron/commands.ts` — только `open` / `action`. Action commands в `commands.ts` **не хардкодятся**: они приходят dynamic от приложений.
- Extension content в `extensions/<id>/` — static (no build step yet, PoC).

## 11. Стиль коммитов и кода

- Коммит — про **почему**, не про **что**. Не «add big».
- Никаких `--no-verify`.
- Не амендь опубликованные коммиты, делай новый.
- Не пиши документацию ради документации; обновляй concept-страницы только если меняешь концепт.
- Минимум комментариев. Понятные имена > комментарии.

См. [Рабочий процесс](/guide/workflow).
