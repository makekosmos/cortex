# Spec: Bug Audit — Wave 2

**Date:** 2026-05-17  
**Scope:** Вторая волна исправлений после глубокого аудита 5 агентами.

## AC-1: mesh.rs HLC dedup — `splitn(3, ':')` возвращает неверный segment

**Файл:** `crates/ark-core/rust/src/mesh.rs:102`

HLC формат: `2026-03-28T14:30:00.123Z:000042:device-id`.  
ISO8601 содержит `:` в часах и минутах. `splitn(3, ':')` делит строку на первые 3 части:
- part[0] = `2026-03-28T14`
- part[1] = `30`
- part[2] = `00.123Z:000042:device-id`

`nth(2)` = `"00.123Z:000042:device-id"` — неправильный device_id, dedup completely broken.

**Fix:** Использовать `HLC::from_string(hlc).device_id`.

**AC PASS when:** тест `dedup_key_extracts_device_id_correctly` проходит.

---

## AC-2: rawg.rs — отсутствует HTTP timeout

**Файл:** `services/kepler-backend/src/arrancador/rawg.rs:113-117`

`reqwest::Client::builder()` без `.timeout()` — запрос может зависнуть бесконечно,
вызвав зависание process или OOM на медленном ответе.

**Fix:** Добавить `.timeout(Duration::from_secs(30))` в `build_client()`.

**AC PASS when:** `build_client` имеет timeout; тест `rawg_timeout_configured` проходит.

---

## AC-3: CSV formula injection в task_csv / time_entry_csv

**Файлы:** `services/kepler-backend/src/export/task_csv.rs`, `time_entry_csv.rs`

Ячейки, начинающиеся с `=`, `+`, `-`, `@`, интерпретируются Excel/Google Sheets
как формулы. Пример: title `=cmd|' /C calc'!A0` → RCE при открытии в Excel.

**Fix:** В `export/mod.rs` добавить функцию `csv_safe_cell(s)` которая префиксирует
табуляцией `\t` значения начинающиеся с `=+-@`. Применить во всех `write_record`.

**AC PASS when:** тест `csv_safe_cell_sanitizes_formula_prefix` проходит; существующие тесты зелёные.

---

## AC-4: extension-host.ts — icon path traversal

**Файл:** `shell/electron/extension-host.ts:372`

`path.join(dir, manifest.icon)` без проверки traversal. Злоумышленник в `.kext`
указывает `icon: "../../shell/electron/preload.ts"` → readFileSync читает файл
за пределами extension directory.

**Fix:** Проверить что `path.resolve(iconPath).startsWith(path.resolve(dir))`.

**AC PASS when:** Code review показывает bounds check перед `readFileSync`.

---

## AC-5: db.rs — replay_pending_for_type не атомарен

**Файл:** `crates/ark-core/rust/src/db.rs:636-656`

`upsert_object` + `DELETE FROM sync_pending_objects` выполняются без транзакции.
Если между ними происходит ошибка, объект вставлен но pending-запись остаётся →
следующий replay вставит объект повторно.

**Fix:** Обернуть цикл в `BEGIN IMMEDIATE` / `COMMIT`.

**AC PASS when:** Код использует транзакцию; тесты в db.rs зелёные.

---

## AC-6: extension-marketplace.ts — неограниченный redirect chain

**Файл:** `shell/electron/extension-marketplace.ts:83-84`

`httpsDownload` рекурсивно следует редиректам без ограничения глубины.
Атакующий relay-сервер может организовать бесконечный цикл редиректов.

**Fix:** Добавить счётчик `redirectCount` с лимитом 5; reject если превышен.

**AC PASS when:** При 6-м редиректе `httpsDownload` rejecting с ошибкой.

---

## AC-7: relay_transport.rs — unbounded offline outbox

**Файл:** `crates/ark-core/rust/src/relay_transport.rs:235`

`outbox` накапливает сообщения при недоступности relay без ограничения размера.
Долгая offline-фаза с активной синхронизацией → OOM.

**Fix:** Добавить константу `MAX_OUTBOX_SIZE = 500`, перед `push_back` дропать
самое старое сообщение если `outbox.len() >= MAX_OUTBOX_SIZE`.

**AC PASS when:** `relay_outbox_bounded` тест проходит.

---

## AC-8: note_md.rs — YAML literal newlines в quoted strings

**Файл:** `services/kepler-backend/src/export/note_md.rs:106`

`yaml_scalar` quotes строки с `\n`, но не escapes сами `\n` → в YAML double-quoted
строке остаётся literal newline, ломающий структуру frontmatter.

**Fix:** В `escaped` также заменять `'\n'` → `"\\n"` и `'\r'` → `"\\r"`.

**AC PASS when:** тест `yaml_scalar_escapes_newlines` проходит.

---

## AC-9: note_md.rs — unsafe markdown link href (javascript: scheme)

**Файл:** `services/kepler-backend/src/export/note_md.rs:319-320`

`href` из TipTap вставляется в markdown напрямую без проверки scheme.
`javascript:alert()` в markdown открытом в некоторых редакторах → XSS.

**Fix:** Пропускать в markdown только URL с `http://` или `https://` prefix.
Остальные href заменять на `#`.

**AC PASS when:** тест `render_unsafe_link_replaced` проходит.

---

## AC-10: scanner.rs — VDF parser stack overflow на глубоко вложенных файлах

**Файл:** `services/kepler-backend/src/arrancador/scanner.rs:156`

`parse_object_body` рекурсивна без ограничения глубины. Вредоносный VDF
с тысячами уровней вложенности → stack overflow.

**Fix:** Добавить параметр `depth: usize`, возвращать `VdfNode::Null` при `depth > 64`.

**AC PASS when:** тест `parse_deeply_nested_vdf_no_overflow` проходит.

---

## AC-11: protocol.rs — silent serialization failure

**Файл:** `crates/ark-core/rust/src/protocol.rs:87`

`serde_json::to_string(msg).unwrap_or_default()` — при ошибке возвращает пустую
строку, которая молча дропается receiver'ом. Потеря sync-сообщения без лога.

**Fix:** Добавить `eprintln!` лог перед возвратом дефолта.

**AC PASS when:** Code review — присутствует лог при сбое.

---

## AC-12: settings-window.ts — non-atomic write

**Файл:** `shell/electron/settings-window.ts:64`

`writeFileSync(path, data)` — прямая запись без atomic rename. Crash mid-write
→ corrupted settings file.

**Fix:** Write to `.tmp` file, then `renameSync` → atomic.

**AC PASS when:** Code review — атомарный write через temp file + rename.

---

## Файлы для изменения

1. `crates/ark-core/rust/src/mesh.rs` (AC-1)
2. `services/kepler-backend/src/arrancador/rawg.rs` (AC-2)
3. `services/kepler-backend/src/export/mod.rs` (AC-3)
4. `services/kepler-backend/src/export/task_csv.rs` (AC-3)
5. `services/kepler-backend/src/export/time_entry_csv.rs` (AC-3)
6. `shell/electron/extension-host.ts` (AC-4)
7. `crates/ark-core/rust/src/db.rs` (AC-5)
8. `shell/electron/extension-marketplace.ts` (AC-6)
9. `crates/ark-core/rust/src/relay_transport.rs` (AC-7)
10. `services/kepler-backend/src/export/note_md.rs` (AC-8, AC-9)
11. `services/kepler-backend/src/arrancador/scanner.rs` (AC-10)
12. `crates/ark-core/rust/src/protocol.rs` (AC-11)
13. `shell/electron/settings-window.ts` (AC-12)
