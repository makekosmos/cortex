# Журнал решений (ADR)

Архитектурные решения, оформленные как отдельные документы в `docs/`. Каждое решение — это «почему сделали именно так», чтобы через год можно было понять контекст.

## Список решений

### Delphi — legacy DB sidecar удалён

Источник: `docs/DELPHI-LEGACY-DB-DECISION.md`

Старый Delphi-specific Rust DB sidecar удалён. Текущая замена для shared task state — ARK object storage:

- task data: `objects` с `type_id = task_obj`
- typed/schema metadata: `object_types`
- relationships: `object_links`
- app access: `@kosmos/ark` / `ark-core-rpc`

Правила:

- Не паковать и не восстанавливать `apps/delphi/ts/sidecar`.
- Не использовать old todo таблицы как long-term fallback после миграции.
- Delphi startup может читать legacy todos **только** для миграции в `task_obj`.
- После миграции `task_obj` — источник правды.

### Eden Heart vs ARK boundary

Источник: `docs/EDEN-HEART-ARK-BOUNDARY.md`

Eden Heart **остаётся**, не удалён, потому что Eden notes — ARK объекты. Разделение:

- **ARK** владеет shared data identity и syncable state: `object_types`, `objects`, `object_links`.
- **Eden Heart** владеет тяжёлой editor/vault-local работой: vault import/export, one-time migration source reads, editor-oriented трансформации, будущий специализированный поиск/индексация.
- Новые Eden note writes идут в ARK objects. Heart **не** permanent fallback для shared note identity/state после startup migration.

Search-решение:

- **ARK search** — поиск по `objects.title`, `objects.content_json`, `objects.props_json`.
- **Heart search** — будущий Eden-specific Rust индекс для editor/vault контента, если ARK object search недостаточно.
- **Текущий дефолт**: shared notes ищутся через ARK; Heart остаётся как опция позже.

Текущее runtime правило:

- `loadEntry`, `listEntries`, `listNoteTypes`, `searchEntries` читают **только** ARK объекты/types.
- Heart entry/type reads — только startup migration, не normal read paths.

### Read-only SQL boundary

Источник: `docs/ARK-READONLY-SQL-BOUNDARY.md`

Две независимые политики:

1. **Write rule** — жёсткий: app TS services **не** пишут напрямую в ARK таблицы.
2. **Read inspection** — мягче:
   - Dashboard — read-only inspector, может открывать любую выбранную ARK SQLite-БД.
   - Arrancador — `@kosmos/ark` сначала; raw SQLite допустим как fallback когда runtime недоступен.
   - Renderer — никогда не открывает SQLite напрямую.

Будущий шаг — заменить оставшиеся fallback SQLite paths специализированными ARK endpoints. Уже добавлены:

- `objects.listByType` → `list_objects_by_type`
- `objects.getMany` → `get_objects_by_ids`
- `usage.processes.recent` → `list_recent_usage_processes`
- `usage.processes.search` → `search_usage_processes`
- `usage.gamePlaytime.summary` → `get_usage_game_playtime_summary`

### 2026-05-14 — Brand swap Kepler ↔ Kosmos

Источник: `docs/MIGRATION-2026-05-14-brand-swap.md`

Поменяли семантику бренда:

- **Kepler** теперь — имя **лаунчера** и его shell (`apps/kepler-shell/`, `services/kepler-backend/`).
- **Kosmos** теперь — имя **экосистемы / монорепо** (`@kosmos/ark`, `@kosmos/visuals`, ARK runtime, документация).

Раньше было наоборот. Все references в коде, конфигах, документации и токенах прошли через `scripts/migrate-kepler-to-kosmos.ps1`. Гард — `scripts/check-swap-completeness.ps1`.

### 2026-05-14 — Apps остаются standalone .exe + shared backend

Решено **не** мигрировать приложения в extensions лаунчера (Phase 1-3 plan отброшен). Каждое приложение по-прежнему — независимый Electron `.exe` со своим окном и пакетом. Kepler-shell вызывает их через command bus; общий backend (`services/kepler-backend/`) хостит command registry и WS server.

Причина: extension model в Phase 1 PoC показал нарастающую сложность (разделяемый renderer, конфликты CSS-токенов, packaging) при минимальной выгоде. Standalone-распространение проще и сохраняет user expectation «отдельная иконка в Start menu на каждое приложение».

### 2026-05-14 — Command bus как primary integration primitive

Apps **регистрируют** свои commands в shared backend (через `@kosmos/ark` SDK), launcher **invoke**'ает их. Это заменяет более ранний план «ARK FTS5 search в launcher» — поиск в Kepler-shell теперь идёт по зарегистрированным командам, а не по индексу заметок/задач.

- Wire format: flat events `{event: "...", ...fields}` (не nested).
- Registration в `kepler-mode` only, под `try/catch`.

### 2026-05-14 — Launcher window: fixed-size 720×460

Лаунчер Kepler — окно фиксированного размера 720×460. Animated resize (per-frame) отброшен после экспериментов: Win32 не успевает синхронно прокидывать события, окно дёргается. Решение — фиксированный размер; expand/collapse состояния выражаются через layout внутри renderer, не через resize окна.

## Шаблон для нового решения

Все новые архитектурные/безопасностные решения **обязаны** попадать сюда. Минимальный шаблон ADR:

```markdown
# <Краткое название>

**Дата:** YYYY-MM-DD
**Статус:** Accepted / Proposed / Superseded by <ADR>

## Контекст

Что было неудобно / не работало / противоречиво.

## Решение

Что выбрали и почему именно так.

## Последствия

- Что улучшится.
- Что усложнится.
- Что точно нельзя делать после этого решения.

## Альтернативы, которые отбросили

- Альтернатива A — почему не подошла.
- Альтернатива B — почему не подошла.
```

Сохраняй в `docs/<KEBAB-CASE-NAME>.md` и добавляй ссылку сюда.
