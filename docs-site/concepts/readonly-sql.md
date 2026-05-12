# Read-only SQL boundary

Источник правды: `docs/ARK-READONLY-SQL-BOUNDARY.md`.

В репо две независимых политики:

1. **Кто может писать в ARK?** Жёстко — только `@kepler/ark` или `ark_core::db`. См. [Граница записи](/concepts/write-boundary).
2. **Кто может читать ARK напрямую?** Свободнее. Это страница про второй вопрос.

## Кому разрешено читать напрямую

| Контекст | Можно? | Условия |
|---|---|---|
| Renderer любого приложения | ❌ нет | Только preload API |
| Electron main | ✅ да | Только `mode=ro`, явный fallback, отделённый от writes |
| Dashboard | ✅ полный read-only inspector | Любая выбранная ARK SQLite-БД |
| Arrancador | ⚠️ ограничено | Только как fallback при недоступном `ark-core-rpc` |
| Eden / Delphi / другие | ⚠️ ограничено | Только специальные пути (миграция, vault-локальная работа) |

## Почему read-only мягче

Read-only SQLite не ломает sync — он ничего не меняет. Поэтому для inspector-режима (Dashboard) и для emergency-fallback'ов это допустимо.

Главное:

- В **renderer** SQLite не открывается **никогда**.
- В **main** read-only path должен быть **визуально отделён** от write path в коде. Не мешай read и write в одной функции.
- Read-only пути **не должны попадать** в автотесты против user DB.

## Dashboard — особый случай

Dashboard — буквально read-only ARK inspector. Это часть его product surface:

- Можно открывать любую выбранную ARK SQLite-БД через Electron main.
- Можно показывать данные в UI.
- **Нельзя** делать `INSERT` / `UPDATE` / `DELETE`.
- Smoke использует `apps/dashboard/.e2e/smoke-dashboard.db`, никогда — user DB.

Когда возможно, Dashboard должен **сначала** пробовать `@kepler/ark` analytics-endpoints:

- `list_objects_by_type`
- `get_objects_by_ids`
- `list_recent_usage_processes`
- `search_usage_processes`
- `get_usage_game_playtime_summary`

Read-only SQLite — fallback, когда подходящего endpoint нет.

## Arrancador — fallback

Arrancador предпочитает `@kepler/ark`:

- hydration `game_obj` объектов
- usage queries
- process search
- playtime aggregation
- backfill

Read-only SQLite используется **только** когда `ark-core-rpc` недоступен (sidecar упал, не собрался, не нашёлся бинарь). Это деградированный режим, не штатный.

## Будущее

Если проект захочет полностью убрать прямые SQLite reads, нужно:

1. Покрыть оставшиеся fallback-пути специализированными ARK endpoints.
2. Удалить read-only fallback из Arrancador.
3. Оставить Dashboard как единственного полноценного inspector (с явным product framing).

Часть endpoints уже добавлена (список выше). Прогресс — в `docs/ARK-READONLY-SQL-BOUNDARY.md`.

## Что ещё нужно

- App-specific **bulk analytics queries**, чтобы не грузить полный usage snapshot ради одной агрегации.
- Возможно, специализированный read-only режим `@kepler/ark` для inspector-приложений.
