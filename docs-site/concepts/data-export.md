# Data export — universal per-type конвертеры

::: tip Phase 7 ✅ (2026-05-18)
ARK данные больше не заперты в SQLite. Single export-API + UI секция в Kepler Settings. Каждый object_type имеет свой плагин-конвертер.
:::

## Зачем

ARK хранит всё в одной `ark.db` (SQLite). Пользователь хочет вытаскивать данные в human-readable формате:

- заметки как markdown (для Obsidian / Bear / git)
- задачи как markdown + CSV
- time entries в CSV для отчётов
- список игр / тегов в JSON

Раньше Eden имел кнопку «Экспорт в Markdown» (через Heart Rust sidecar). После Phase 6.0.A удалено — функция переехала сюда как универсальный механизм.

## Архитектура

```
shell/src/views/SettingsView.vue (UI section "Экспорт")
        ↓ window.kepler.export.{list, run, pickDir}
shell/electron/preload.ts (contextBridge)
        ↓ ipcRenderer.invoke("kepler:export:*")
shell/electron/main.ts (IPC handlers)
        ↓ arkClient.invokeOperation({operation: "export.list" | "export.run"})
services/kepler-backend/src/ws_server.rs (export.* dispatch)
        ↓
services/kepler-backend/src/export/mod.rs (Converter registry)
        ↓ для каждого конвертера: list_objects_by_type → convert → write files
        files written в dest_dir
```

## Converter trait

```rust
pub trait Converter: Send + Sync {
    fn id(&self) -> &str;                   // "note_md"
    fn display_name(&self) -> &str;         // "Заметки → Markdown"
    fn object_type(&self) -> &str;          // "note_obj"
    fn default_format(&self) -> &str;       // "md"
    fn supported_formats(&self) -> &[&str]; // &["md"]
    fn convert(
        &self,
        objects: &[ArkObject],
        format: &str,
        dest_dir: &Path,
    ) -> ConvertResult;
}

pub struct ConvertResult {
    pub files_written: Vec<PathBuf>,
    pub bytes: u64,
    pub errors: Vec<String>,
}
```

Registry — `OnceLock<Vec<Box<dyn Converter>>>` в `export/mod.rs`. Регистрация — на boot kepler-backend.

## Текущие конвертеры (6)

| Converter id | Object type | Format | Файлы |
|---|---|---|---|
| `note_md` | `note_obj` | `md` | `<title>.md` per note + YAML frontmatter (id/type/title/created/updated/header_props). Body — TipTap JSON → markdown (paragraph, heading 1-6, lists, code_block, blockquote, marks: bold/italic/code/link). |
| `task_md` | `task_obj` | `md` | `<title>.md` per task + frontmatter (project_id, area_id, scheduled_date, deadline, priority, completed, tags). Body — notes + `## Чек-лист` с `- [x]` / `- [ ]`. |
| `task_csv` | `task_obj` | `csv` | Один `tasks.csv`. Columns: id, title, project_id, area_id, scheduled_date, deadline, completed, priority, tags (joined `\|`). |
| `time_entry_csv` | `time_entry_obj` | `csv` | Один `time-entries.csv`. Columns: id, title, started_at, ended_at, duration_minutes, task_id, source. |
| `tag_json` | `tag_obj` | `json` | `tags.json` — pretty-printed array всех tag_obj. |
| `game_json` | `game_obj` | `json` | `games.json` — pretty-printed array всех game_obj. |

## WS API

### `export.list`

Без параметров. Возвращает массив всех зарегистрированных конвертеров:

```json
{
  "converters": [
    {
      "converter_id": "note_md",
      "object_type": "note_obj",
      "display_name": "Заметки → Markdown",
      "default_format": "md",
      "supported_formats": ["md"]
    },
    ...
  ]
}
```

### `export.run`

```json
// request
{ "converter_id": "note_md", "format": "md", "dest_dir": "D:\\export\\notes" }

// response
{ "files_written": ["D:\\export\\notes\\hello.md", ...], "bytes": 12345, "errors": [] }
```

- `format` опционален — берётся `default_format`.
- `dest_dir` создаётся если не существует (рекурсивно).
- Filename collision → суффикс `-2`, `-3`, ... до 10k.
- Sanitize filename: убираются `/\:*?"<>|`, обрезается до 200 chars.

## UI

`shell/src/views/SettingsView.vue` → таб «Экспорт»:

- Список converters (`export.list`).
- Per-converter карточка: display_name, dropdown формата (если `supported_formats.length > 1`), кнопка «Экспортировать».
- Click → `pickDir` IPC → native directory picker → `export.run` → inline статус.
- «Последние экспорты» (10 шт) в localStorage `kepler-export-history`.

## Как добавить новый конвертер

1. Создать `services/kepler-backend/src/export/<name>.rs` с `pub struct MyConverter` + `impl Converter`.
2. Добавить в `export/mod.rs::list_converters()` `Box::new(<name>::MyConverter)`.
3. Минимум 2 unit test'а в `<name>.rs` (`#[cfg(test)] mod tests`): создать sample ArkObject, вызвать `convert(...)`, assert файлы созданы + содержат ключевые строки.
4. `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib` зелёный.
5. UI автоматически подхватит новый конвертер через `export.list`.

## Что НЕ делает Phase 7

- **Import обратно** из markdown / CSV. Нужны collision rules + merge strategy — отдельная Phase 7.X если потребуется.
- **Filter API** (по date range, tags). Сейчас экспортируется всё. Phase 7.1 если понадобится.
- **Encryption / passphrase**. User сам зипует с passphrase при необходимости.
- **Scheduled export** (cron). Backup в Phase 11 покрывает регулярный snapshot ARK DB.
- **GPX для `run_obj`**. Тип ещё не существует (требует Olympia extension).

## Связанные документы

- [Модель данных ARK](/concepts/ark-objects)
- [Граница записи](/concepts/write-boundary) — export read-only, не нарушает invariant
- [Backup roadmap](/apps/kepler-roadmap#phase-11-backup) — отдельный механизм
