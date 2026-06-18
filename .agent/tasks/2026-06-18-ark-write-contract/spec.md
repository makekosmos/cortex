# ARK write-RPC: fail-closed + атомарность entity/sync-meta

Дата: 2026-06-18 · Классификация: FULL_LOOP (write-boundary + sync)

## Проблема

Все 21 write-хендлер в `core/ark/crates/ark-core/rust/src/main.rs` используют паттерн:

```rust
let result = with_conn(|conn| { db::upsert_x(conn, &x)?; record_local_upsert(conn, ...)?; Ok(entity) });
if let Ok(entity) = result {
    tokio::spawn(async move { broadcast_local_change(entity).await; });
    // для object: emit_event(...)
}
Ok(json!(true))   // <-- возвращает успех ДАЖЕ если result == Err
```

Два связанных дефекта:

1. **False-success.** Если `with_conn` вернул `Err` (DB/constraint/serialize), ошибка молча
   проглатывается, broadcast/emit не происходит, но RPC отвечает `Ok(json!(true))`.
   Клиент (TS SDK) считает запись успешной → silent data loss, расхождение UI/БД, пропуск
   sync-события.
2. **Неатомарность entity + sync-meta.** `with_conn` (main.rs:449) НЕ открывает внешнюю
   транзакцию. `db::upsert_object` (db.rs:951) оборачивает entity+FTS в собственный savepoint
   и RELEASE'ит его (= commit в autocommit-режиме) ДО того, как выполнится
   `record_local_upsert`. Если `record_local_upsert` упадёт — объект уже зафиксирован, а
   version-vector нет. Entity и sync-метаданные могут разойтись.

## Инвариант (цель)

Для каждой ARK write-RPC:

> либо (entity-row + FTS + sync version-vector/tombstone) записаны и закоммичены все вместе,
> либо не записано ничего. `broadcast_local_change` / `emit_event` — строго ПОСЛЕ commit.
> RPC возвращает `Err` при любой ошибке записи.

## Дизайн

Ввести один хелпер в `main.rs` рядом с `with_conn`:

```rust
/// Выполняет write-замыкание в одной SQLite-транзакции поверх shared conn.
/// COMMIT при Ok, ROLLBACK при Err. Гарантирует атомарность entity + FTS + sync-meta:
/// вложенные SAVEPOINT внутри db::* работают внутри этого BEGIN, при ошибке откатывается всё.
fn with_write_tx<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String>,
{
    with_conn(|conn| {
        conn.execute_batch("BEGIN IMMEDIATE").map_err(|e| e.to_string())?;
        match f(conn) {
            Ok(v) => {
                conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
                Ok(v)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    })
}
```

Переписать каждый из 21 хендлера по шаблону:

```rust
// было: let result = with_conn(...); if let Ok(entity) = result { broadcast }; Ok(json!(true))
let entity = with_write_tx(|conn| { db::upsert_x(conn, &x)?; record_local_upsert(conn, ...)?; Ok(entity) })?;
tokio::spawn(async move { broadcast_local_change(entity).await; });
Ok(json!(true))
```

Частные случаи:

- `UpsertObject` / `DeleteObject`: `emit_event(object_upserted/object_deleted)` тоже ПОСЛЕ `?`
  (только при успехе). Клоны `eid`/`etid` снять до tx, как сейчас.
- `BatchUpsertTodos`: весь батч в одной `with_write_tx` (бонус — атомарность всего батча);
  `tokio::spawn` цикла broadcast после `?`.

Список 21 хендлера (имя — строки): UpsertTodo 649, DeleteTodo 670, BatchUpsertTodos 684,
UpsertProject 710, DeleteProject 730, UpsertArea 744, UpsertTag 764, UpsertHeading 784,
DeleteHeading 804, UpsertTrackedApp 818, DeleteTrackedApp 841, UpsertUsageSession 861,
DeleteUsageSession 884, UpsertUsageEvent 904, DeleteUsageEvent 927, UpsertObject 1012,
DeleteObject 1044, UpsertObjectType 1071, DeleteObjectType 1093, UpsertObjectLink 1116,
DeleteObjectLink 1138.

Вне scope (уже fail-closed, single-statement): `SetSyncKv`, `ClearAll`, `DeleteTrashed`.

## Тест-план (RED до реализации)

Новые `#[tokio::test]` в `#[cfg(test)]` секции main.rs, по образцу
`upsert_object_broadcasts_live_change_to_peers` (стр. 3054), с `TEST_DB_MUTEX` + tempfile + Init.

1. `upsert_object_with_invalid_type_id_is_err` — объект с несуществующим `type_id`
   (FK violation) → `handle_request(Request::UpsertObject{..})` возвращает `Err`.
   Текущий код возвращает `Ok(true)` → **RED**. Ловит дефект №1 напрямую.
2. `failed_upsert_object_persists_nothing` — после неудачного upsert строки в `objects`
   с этим id нет И нет записи в sync-таблице version-vector для этого id (атомарность,
   дефект №2). На текущем коде объект тоже не сохранится (FK падает в db::upsert_object),
   но тест фиксирует инвариант на будущее и проверяет отсутствие частичной sync-записи.
3. (позитив, регрессия) существующие `upsert_object_broadcasts_*` должны остаться зелёными —
   успешный путь с валидным type_id по-прежнему пишет объект + sync-row + broadcast.

Примечание: прямой кейс «entity ok, sync-meta fail» требует fault injection и вне scope
unit-теста; атомарность этого участка покрывается конструктивно (`with_write_tx`).

## Фаза B — полная атомарность BatchUpsertTodos / UpsertObjectType

В фазе A эти 2 хендлера остались на `with_conn + ?` (fail-closed, но sync-meta вне tx),
т.к. их db-функции открывают свой `BEGIN`. Доводим до полной атомарности.

Разведка call-sites: `batch_upsert_todos` (ffi.rs:229, main.rs:706, тест db.rs:3624) и
`replay_pending_for_type` (только из `upsert_object_type`, db.rs:707) — ВСЕ в autocommit.
`upsert_object_type` сам транзакцию не открывает; единственный источник `BEGIN` —
`replay_pending_for_type`. Замена `BEGIN→SAVEPOINT` безопасна для всех call-sites.
FK на `todos` нет.

Дизайн:

- `db::batch_upsert_todos` (db.rs:545): `BEGIN`→`SAVEPOINT ark_batch_upsert_todos`,
  ошибка→`rollback_savepoint(conn, "ark_batch_upsert_todos")`, успех→`RELEASE SAVEPOINT ...`.
- `db::replay_pending_for_type` (db.rs:774): `BEGIN IMMEDIATE`→`SAVEPOINT ark_replay_pending`,
  COMMIT→`RELEASE`, ROLLBACK→`rollback_savepoint(conn, "ark_replay_pending")`.
  (Паттерн как в `db::upsert_object`, savepoint `ark_upsert_object` вкладывается внутри — ок.)
- main.rs: `BatchUpsertTodos` и `UpsertObjectType` — `with_conn`→`with_write_tx`,
  обновить комментарии (теперь атомарно).

RED (тестируемо без fault injection): до замены вызов функции внутри внешней транзакции
падает с "cannot start a transaction within a transaction":

- `batch_upsert_todos_nests_in_outer_transaction` (db.rs): открыть `BEGIN IMMEDIATE`,
  вызвать `batch_upsert_todos`, ожидать `Ok` → до фазы B FAIL, после OK.
- `upsert_object_type_nests_in_outer_transaction` (db.rs): аналогично.
  GREEN: оба зелёные + регрессия (18 main.rs тестов + db.rs object_type/todo тесты).

## Проверки

- `cargo test` для пакета ark-core-rpc + db.rs тесты (требует ЗАКРЫТОГО Kosmos — beacon-порт + lock на exe).
- RED показать ДО реализации, GREEN после.
- Один логический change → один коммит. Без попутного рефакторинга.
