# Evidence: Bug Audit Fixes

**Date:** 2026-05-17  
**Branch:** claude/audit-project-bugs-RhvQl

## AC-1: sanitize_filename не паникует на UTF-8 строках > 120 байт

**Status: PASS**

Изменение: `services/kepler-backend/src/export/mod.rs` — заменено `out.truncate(120)` на
цикл поиска char boundary перед усечением.

Тест `sanitize_filename_truncates_at_char_boundary` проходит:

- 61 кириллический символ = 122 байта → truncate(120) паниковал бы без fix
- После fix: усекает до 60 символов = 120 байт, валидный UTF-8

Вывод `cargo test --lib -- export`:

```
test export::tests::sanitize_filename_truncates_at_char_boundary ... ok
test export::tests::sanitize_filename_strips_banned_chars ... ok
```

---

## AC-2: HLC::from_string не паникует на пустой строке

**Status: PASS**

Изменение: `crates/ark-core/rust/src/hlc.rs` — добавлены early returns при отсутствии
'Z' или ':' вместо panic-prone slicing через `unwrap_or(0)`.

Новые тесты проходят:

```
test hlc::tests::test_from_string_empty ... ok
test hlc::tests::test_from_string_no_colons ... ok
test hlc::tests::test_from_string_one_colon_after_z ... ok
```

Все 15 HLC тестов зелёные.

---

## AC-3: SQOBA restore блокирует абсолютные пути

**Status: PASS**

Изменение: `services/kepler-backend/src/arrancador/sqoba.rs` — добавлена проверка
`std::path::Path::new(rel).is_absolute()` и belt-and-suspenders проверка
`!target.starts_with(&root)` после join.

Тест `sqoba_restore_rejects_absolute_path_in_zip` проходит:

- Zip с entry `source_0//evil.txt` (rel="/evil.txt") → rejected
- Файл не записан за пределами root
- Легитимная запись `source_0/save.dat` восстановлена (restored_files=1)

Все 9 sqoba тестов зелёные.

---

## AC-4: Scan dedup логирует ошибку вместо silent swallow

**Status: PASS**

Изменение: `services/kepler-backend/src/ws_server.rs` — заменён `unwrap_or_default()` на
явный `match` с `eprintln!`-логом и возвратом ошибки клиенту.

Code review: при сбое десериализации ARK-ответа клиент получает ошибку
`"arrancador.scan: failed to parse existing games: ..."` вместо молчаливого продолжения
с пустым списком.

---

## AC-5: unique_path не перезаписывает при истощении суффиксов

**Status: PASS**

Изменение: `services/kepler-backend/src/export/mod.rs` — fallback изменён с `stem.ext`
(уже существует) на `stem-{epoch_millis}.ext`.

Тест `test_unique_path_exhaustion_fallback` проходит:

- Создаёт 10,000 файлов (stem.ext + stem-2.ext … stem-9999.ext)
- Проверяет что `unique_path` возвращает несуществующий путь

---

## AC-6: rawg_id u64→u32 — явная проверка

**Status: PASS**

Изменение: `services/kepler-backend/src/ws_server.rs` — заменён `n as u32` на
`u32::try_from(n)` с возвратом ошибки при переполнении.

Code review: При rawg_id > 4,294,967,295 клиент получает ошибку
`"arrancador.rawg.apply: rawg_id {n} exceeds u32 range"`.

---

## Итоговые прогоны тестов

```
cargo test --manifest-path services/kepler-backend/Cargo.toml --lib
→ 108 passed; 0 failed

cargo test --manifest-path crates/ark-core/rust/Cargo.toml --lib
→ 147 passed; 0 failed
```

Все AC: PASS.
