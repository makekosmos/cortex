# Spec: Bug Audit Fixes

**Date:** 2026-05-17  
**Scope:** Исправление реальных багов, найденных при аудите кодовой базы.

## Контекст

Аудит трёх слоёв кодовой базы (Rust backend, TypeScript/Electron shell, ARK core) выявил
несколько реальных багов. Данная задача — их исправить.

## Acceptance Criteria

### AC-1: `sanitize_filename` не паникует на UTF-8 строках > 120 байт

**Файл:** `services/kepler-backend/src/export/mod.rs`

`String::truncate(120)` паникует, если байтовый индекс 120 не находится на границе
UTF-8-символа. Для строк с кириллицей/китайскими символами: 61 символ = 122 байта →
`truncate(120)` обрежет половину символа → паника.

**Fix:** заменить `out.truncate(120)` на усечение по символьной границе.

**Тест:** `sanitize_filename("А".repeat(70))` должен возвращать строку ≤ 120 байт без паники.

**AC PASS when:** тест `sanitize_filename_truncates_at_char_boundary` проходит; `cargo test` зелёный.

---

### AC-2: `HLC::from_string` не паникует на пустой строке

**Файл:** `crates/ark-core/rust/src/hlc.rs`

При вызове `HLC::from_string("")`:

- `z_pos = 0`, `first_colon = 0`
- `rest = &s[1..]` → паника: byte index 1 out of bounds for "" (len=0)

Аналогично для строк длиной < 1 байт когда 'Z' или ':' не найдены.

**Fix:** защитить индексы перед слайсингом. Если строка не соответствует формату,
возвращать дефолтный HLC вместо паники.

**AC PASS when:** тест `test_from_string_empty` и `test_from_string_no_colons` проходят; `cargo test` зелёный.

---

### AC-3: SQOBA restore блокирует абсолютные пути

**Файл:** `services/kepler-backend/src/arrancador/sqoba.rs`

Текущая проверка `if rel.contains("..")` блокирует только traversal через `..`,
но не блокирует абсолютные пути. В Rust `PathBuf::join("/etc/passwd")` возвращает
`/etc/passwd`, полностью игнорируя root. Атакующий, создавший вредоносный zip с
записью `source_0//etc/passwd`, получит запись файла в `/etc/passwd`.

**Fix:** добавить проверку, что `rel` не является абсолютным путём
(`Path::new(rel).is_absolute()`), и проверить что `target` остаётся внутри `root`
после `join`.

**AC PASS when:** тест `test_restore_rejects_absolute_path` проходит; существующие
sqoba-тесты остаются зелёными.

---

### AC-4: Scan dedup логирует ошибку десериализации вместо silent swallow

**Файл:** `services/kepler-backend/src/ws_server.rs`

```rust
serde_json::from_value(ark_resp.data).unwrap_or_default()
```

При сбое десериализации ARK-ответа `arrancador.scan` молча начинает с пустым списком
существующих игр и дублирует все игры при следующем скане.

**Fix:** заменить `unwrap_or_default()` на обработку ошибки с явным логом и возвратом
ошибочного ответа, либо хотя бы `eprintln!`-логом перед дефолтом.

**AC PASS when:** при невалидном ARK-ответе код либо возвращает ошибку клиенту,
либо логирует ошибку перед продолжением. Тест или code review.

---

### AC-5: `unique_path` не перезаписывает файл при истощении суффиксов

**Файл:** `services/kepler-backend/src/export/mod.rs`

Когда все `stem-2.ext` … `stem-9999.ext` заняты, функция возвращает `stem.ext`,
который уже существует → тихая перезапись файла.

**Fix:** вернуть `stem-{timestamp}.ext` или другой гарантированно уникальный суффикс
как fallback вместо потенциально-конфликтного `stem.ext`.

**AC PASS when:** тест `test_unique_path_exhaustion_fallback` проходит, существующие
тесты остаются зелёными.

---

### AC-6: rawg_id u64→u32 — явная проверка вместо silent truncation

**Файл:** `services/kepler-backend/src/ws_server.rs`

```rust
Some(n) => n as u32,
```

RAWG ID > u32::MAX (~4 млрд) молча усечётся до неверного значения.
В настоящее время RAWG IDs малы, но это ненадёжно.

**Fix:** добавить `u32::try_from(n)` с явным возвратом ошибки при переполнении.

**AC PASS when:** код использует `u32::try_from(n)` и возвращает ошибку при overflow.

---

## Что НЕ входит в скоуп

- Рефакторинг за пределами указанных файлов
- Изменения схемы БД
- UI изменения
- Нетривиальные рефакторы Rust-кода
- Tombstone HLC string comparison (FALSE POSITIVE — формат HLC с zero-padded counter корректно сравнивается лексикографически)
- FTS5 DISTINCT (FALSE POSITIVE — index_object_for_search делает DELETE+INSERT, дублей нет)

## Файлы для изменения

1. `services/kepler-backend/src/export/mod.rs` (AC-1, AC-5)
2. `crates/ark-core/rust/src/hlc.rs` (AC-2)
3. `services/kepler-backend/src/arrancador/sqoba.rs` (AC-3)
4. `services/kepler-backend/src/ws_server.rs` (AC-4, AC-6)
