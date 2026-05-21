# 2026-05-18 phase-7-universal-export

## Context

Сейчас данные ARK заперты в SQLite. Пользователь не может вытащить заметки как markdown, тренировки как GPX, задачи как CSV. Eden раньше имел кнопку «Экспорт в Markdown» (через Heart Rust sidecar) — удалена в Phase 6.0.A с расчётом на это решение.

Phase 7 — единый export-API в kepler-backend + UI секция в Settings Kepler shell. Per-type конвертеры реализованы как plugins в Rust. Один раз написано — все extensions получают «Экспорт» бесплатно через shared UI.

## Scope

В задаче:

**Backend (`services/kepler-backend/src/export/`):**

- Модуль `export/mod.rs` — registry конвертеров + dispatcher.
- Trait `Converter`: `id() -> &str`, `display_name() -> &str`, `object_type() -> &str`, `default_format() -> &str`, `supported_formats() -> &[&str]`, `convert(objects: &[ArkObject], format: &str, dest_dir: &Path) -> ConvertResult`.
- `ConvertResult { files_written: Vec<PathBuf>, bytes: u64, errors: Vec<String> }`.
- 5 первых конвертеров:
  - `note_md.rs` — `note_obj` → markdown. Один файл на заметку. Frontmatter (id, type, created_at, updated_at, title, header_props). Body — конвертация TipTap JSON в markdown (через простой recursive walk: paragraph, heading, bullet_list, code_block, и т.д.).
  - `task_md.rs` — `task_obj` → markdown. Один файл на задачу + frontmatter (id, project_id, scheduled_date, deadline, priority, tags). Body — notes + checklist.
  - `task_csv.rs` — `task_obj` → CSV. Колонки: id, title, project_id, area_id, scheduled_date, deadline, completed, priority, tags. Один файл `tasks.csv`.
  - `time_entry_csv.rs` — `time_entry_obj` → CSV. Колонки: id, title, started_at, ended_at, duration_min, task_id, source. Один файл `time-entries.csv`.
  - `tag_json.rs` + `game_json.rs` — `tag_obj` / `game_obj` → JSON (`tags.json` / `games.json`). Просто сериализация всех объектов нужного типа.
- WS endpoint `export.list` → `[{converter_id, object_type, display_name, formats}]`.
- WS endpoint `export.run { converter_id, format, dest_dir }` → выполняет конвертер, возвращает `ConvertResult`. Streaming progress через events `export_progress { converter_id, files_done, files_total }` — опционально, если просто.
- Rust unit tests: каждый конвертер на ≥2 sample object'ах, проверяющие что файлы созданы + содержат ожидаемые поля.

**Frontend (`shell/`):**

- `shell/electron/preload.ts` — exposes `window.kepler.export.{list, run, onProgress}`.
- `shell/src/views/SettingsView.vue` (или новый `ExportPanel.vue` секция) — UI:
  - Список доступных converters (из `export.list`).
  - Per-converter dropdown «Формат» (если несколько форматов).
  - Path picker через dialog API (existing).
  - Кнопка «Экспортировать».
  - Status bar / list последних экспортов (опционально — только если есть `export-history.json` storage).
- Unit/integration smoke в shell (опционально).

**Документация:**

- `docs-site/apps/kepler-roadmap.md` — Phase 7 ✅, замена «⏳».
- `docs-site/concepts/` — новая страница `data-export.md` (как работает converter registry, как добавить новый).
- `STATUS.md` — раздел Phase 7 в Сделано.
- `bun run docs:sync && bun run docs:check` зелёные.

Не в задаче:

- Sync-protocol для export (нет, это локальный one-shot).
- Restore / import обратно из markdown — Phase 7.X отдельный, нужны collision rules.
- GPX converter для `run_obj` — пока нет такого type'а (Olympia сценарий, отдельный proof loop).
- Encryption экспорт-файлов — за scope, при необходимости user сам зипует с passphrase.
- Cron / scheduled export — Phase 11 (backup) перекрывает.

## Acceptance Criteria

**AC1.** `services/kepler-backend/src/export/mod.rs` определяет trait `Converter` + registry. Module экспортирован из `services/kepler-backend/src/lib.rs`.

**AC2.** 5 converters (`note_md`, `task_md`, `task_csv`, `time_entry_csv`, `tag_json`, `game_json`) реализованы, каждый зарегистрирован в registry, каждый имеет ≥2 unit tests.

**AC3.** `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib` — все тесты включая новые converter tests зелёные.

**AC4.** WS endpoint `export.list` возвращает массив всех зарегистрированных converters с метадатой (id, object_type, display_name, formats).

**AC5.** WS endpoint `export.run { converter_id, format, dest_dir }` выполняет export. Test: создаёт isolated dest_dir под `tests/.e2e/<slug>/`, prepopulate ARK 5 sample objects, вызывает `export.run`, проверяет, что в dest_dir появились файлы с ожидаемым содержимым.

**AC6.** `shell/electron/preload.ts` exposes `window.kepler.export.{list, run}` (+ опционально `onProgress`).

**AC7.** `shell/src/views/SettingsView.vue` (или новый ExportPanel) показывает список converters, dropdown format'ов, кнопку «Экспортировать» с path picker. Manual smoke в dev mode возможен.

**AC8.** `bun run --cwd shell typecheck` — clean.

**AC9.** `bun run --cwd shell build:js` — clean.

**AC10.** `bun run ark:guard:writes` — clean (export — read-only от ARK).

**AC11.** `docs-site/concepts/data-export.md` создан, `STATUS.md` + `kepler-roadmap.md` обновлены, `docs:sync && docs:check` зелёные.

## Verification commands

См. AC. Smoke run guidance для оператора — открыть Kepler в dev, Settings → Export, экспортнуть note_obj в `tests/.tmp/export-smoke/`, проверить наличие `<title>.md` файлов.

## Out of scope decisions

- **Streaming progress**: если простой синхронный return ConvertResult достаточен — оставляем sync. WS event `export_progress` опционально, добавляется если конвертация long (>1s).
- **Filter API**: пока экспортируем все объекты нужного типа. Filter по date range / tags — Phase 7.1 если потребуется.
- **Conflict resolution при overwrite**: если в dest_dir уже есть файл — overwrite молча. Confirmation — UI-level, не backend.
- **Filename collisions**: если 2 заметки с одинаковым title — суффикс `-2.md`, `-3.md`. Реализуется в каждом converter'е, не в framework'е.
- **Markdown frontmatter формат**: YAML (как Obsidian / Hugo). Не TOML.
- **CSV dialect**: RFC 4180 (`,` separator, `"` quote, `\r\n` line ending). Standard `csv` crate в Rust.
