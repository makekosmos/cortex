# Журнал решений (ADR)

Архитектурные решения, оформленные как отдельные документы в `docs/`. Каждое решение — это «почему сделали именно так», чтобы через год можно было понять контекст.

## Список решений

### Delphi — legacy DB sidecar удалён

Источник: `docs/DELPHI-LEGACY-DB-DECISION.md`

Старый Delphi-specific Rust DB sidecar удалён. Текущая замена для shared task state — ARK object storage:

- task data: `objects` с `type_id = task_obj`
- typed/schema metadata: `object_types`
- relationships: `object_links`
- app access: `@kepler/ark` / `ark-core-rpc`

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
   - Arrancador — `@kepler/ark` сначала; raw SQLite допустим как fallback когда runtime недоступен.
   - Renderer — никогда не открывает SQLite напрямую.

Будущий шаг — заменить оставшиеся fallback SQLite paths специализированными ARK endpoints. Уже добавлены:

- `objects.listByType` → `list_objects_by_type`
- `objects.getMany` → `get_objects_by_ids`
- `usage.processes.recent` → `list_recent_usage_processes`
- `usage.processes.search` → `search_usage_processes`
- `usage.gamePlaytime.summary` → `get_usage_game_playtime_summary`

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
