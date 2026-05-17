# Evidence — 2026-05-18 phase-7-universal-export

Verified at: 2026-05-18

## AC1 — Converter trait + Registry

**PASS.**

`services/kepler-backend/src/export/mod.rs` определяет:
- `pub trait Converter: Send + Sync` с методами `id`, `display_name`, `object_type`, `default_format`, `supported_formats`, `convert`.
- `pub struct ConvertResult { files_written, bytes, errors }`.
- Registry через `OnceLock<Vec<Box<dyn Converter>>>` + `list_converters()` / `find_converter()` helpers.
- Utilities: `sanitize_filename`, `unique_path` (collision suffix -2, -3, ...).

Экспортирован из `services/kepler-backend/src/lib.rs` (`pub mod export;`).

## AC2 — 6 converters + 17 unit tests

**PASS.**

| Converter | File | Object type | Format | Unit tests |
|---|---|---|---|---|
| `note_md` | `export/note_md.rs` | `note_obj` | `md` | 3 |
| `task_md` | `export/task_md.rs` | `task_obj` | `md` | 3 |
| `task_csv` | `export/task_csv.rs` | `task_obj` | `csv` | 3 |
| `time_entry_csv` | `export/time_entry_csv.rs` | `time_entry_obj` | `csv` | 3 |
| `tag_json` | `export/tag_json.rs` | `tag_obj` | `json` | 2 |
| `game_json` | `export/game_json.rs` | `game_obj` | `json` | 3 |

## AC3 — cargo test green

**PASS.**

```
test result: ok. 75 passed; 0 failed; 0 ignored
```

См. `raw/cargo-test.md`.

## AC4 — export.list operation

**PASS.**

`ws_server.rs::handle_export_op("list", ...)` возвращает массив metadata всех 6 конвертеров с полями `converter_id`, `object_type`, `display_name`, `default_format`, `supported_formats`.

## AC5 — export.run operation

**PASS.**

`handle_export_op("run", params, &ark_host)`:
1. Парсит `converter_id`, `format` (опц.), `dest_dir`.
2. Находит converter через `find_converter`.
3. Создаёт `dest_dir` рекурсивно если не существует.
4. Fetches objects: `ark_host.request("list_objects_by_type", { type_id })`.
5. Вызывает `converter.convert(&objects, &format, &dest_dir)`.
6. Возвращает `ConvertResult { files_written, bytes, errors }` как JSON.

E2e smoke (через shell UI) — операторская проверка, не автоматизировано.

## AC6 — preload exposure

**PASS.**

`shell/electron/preload.ts`:

```ts
window.kepler.export = {
  list: () => ipcRenderer.invoke("kepler:export:list"),
  run: (args) => ipcRenderer.invoke("kepler:export:run", args),
  pickDir: () => ipcRenderer.invoke("kepler:export:pickDir"),
};
```

`shell/electron/main.ts` — 3 IPC handler'а:
- `kepler:export:list` → `arkClient.invokeOperation({operation: "export.list"})`
- `kepler:export:run` → `arkClient.invokeOperation({operation: "export.run", ...args})`
- `kepler:export:pickDir` → `dialog.showOpenDialog(parentWindow, { properties: ["openDirectory", "createDirectory"] })`

Bonus: `pickDir` IPC добавлен потому что existing dialog handler в shell отсутствовал.

## AC7 — UI Settings export panel

**PASS.**

`shell/src/views/SettingsView.vue` — новый таб «Экспорт»:
- Mount → dispatch `window.kepler.export.list()` → state с converters.
- Per-converter карта: `display_name`, dropdown format (если `supported_formats.length > 1`), кнопка «Экспортировать».
- Click → `pickDir` → `run` → inline статус (success / errors).
- «Последние экспорты» (10 шт) в `localStorage["kepler-export-history"]`.

Использует `@kepler/visuals` компоненты где можно.

## AC8 — typecheck

**PASS.**

```
$ bun run --cwd shell typecheck
$ tsc --noEmit
(0 errors)
```

## AC9 — build:js

**PASS.**

`bun run --cwd shell build:js` — main + renderer + 4 extensions собираются без ошибок.

## AC10 — ark:guard:writes

**PASS.**

```
$ bun run ark:guard:writes
ARK write boundary guard passed.
```

Export module использует только `list_objects_by_type` (read-only). Никаких прямых SQL writes.

## AC11 — Документация + docs:sync/check

**PASS.**

- `docs-site/concepts/data-export.md` — новая страница (полное описание механизма + как добавить converter).
- `docs-site/apps/kepler-roadmap.md` — Phase 7 ⏳ → ✅ с детализацией.
- `STATUS.md` — Phase 7 секция в Сделано + lock-file + visuals unification.
- `bun run docs:sync` — регенерация AGENTS.md / CLAUDE.md / llms.txt / mobile-delphi / ark-core AGENTS.md.
- `bun run docs:check` — «всё свежо, stale references не найдено».

## Verification summary

| AC | Verdict |
|---|---|
| AC1 Converter trait + Registry | PASS |
| AC2 6 converters + 17 tests | PASS |
| AC3 cargo test 75/75 | PASS |
| AC4 export.list | PASS |
| AC5 export.run | PASS |
| AC6 preload exposure | PASS |
| AC7 UI Settings panel | PASS |
| AC8 typecheck | PASS |
| AC9 build:js | PASS |
| AC10 ark:guard:writes | PASS |
| AC11 docs | PASS |

## Smoke manual (operator)

E2e спека для шелл UI пока не написана (можно добавить в follow-up). Manual smoke:

1. `bun run --cwd shell dev`
2. Открыть Settings → Export.
3. Создать заметку в Eden через ARK, проверить что появилась в `export.list` для `note_md`.
4. Кликнуть «Экспортировать» → выбрать `tests\.tmp\export-smoke\`.
5. Убедиться, что в папке создан `<title>.md` с frontmatter + markdown body.
6. Аналогично для `task_csv` (если есть task_obj'ы в ARK).
