# ARK / data / sync запреты

::: tip Узкий файл
Читайте только когда задача касается этой области. Полный legacy reference: `docs-site/agents/forbidden.md`.
:::

## ARK writes

- ❌ **Прямой SQL `INSERT` / `UPDATE` / `DELETE`** в `objects`, `object_types`, `object_links`, `tracked_apps`, `usage_sessions`, `usage_events`, `sync_kv` из **app TS services**.
- ❌ **Открытие ARK SQLite на запись** в app services через `better-sqlite3`, `sqlite3`, `node:sqlite` и т.п.
- ❌ Renderer открывает SQLite (любой) напрямую.
- ❌ `INSERT OR REPLACE` для `object_types` / `objects` / `object_links` в `ark_core::db`. SQLite `REPLACE` = delete+insert, а FK `ON DELETE CASCADE` сносит зависимые строки. Только `INSERT ... ON CONFLICT(id) DO UPDATE`.

## Rust panic / Mutex discipline

- ❌ `Mutex::lock().unwrap()` в production code paths (вне `#[cfg(test)]`). Используй `lock().unwrap_or_else(|e| e.into_inner())` для poison recovery — SQLite transactions atomic, данные внутри guard'а валидны после panic'а другого thread'а. Без recovery один panic делает sync неработоспособным **forever до process restart**. См. [DB resilience](/concepts/db-resilience#mutex-poison-recovery).
- ❌ Удалять `crash_reporter::install(data_dir)` из `platform/runtime/src/main.rs::setup`. Это early init для panic_hook'а — без него panic'ы остаются только в stderr (который пропадает после process exit), real user не сможет прислать artifact.
- ❌ Spawn'ить `kepler-backend` без `RUST_BACKTRACE=1` env. Backtrace в crash log = readability для diagnosability.
- ❌ Снижать `BACKEND_MAX_CRASH_STREAK` ниже 3 или удалять supervisor logic в `platform/desktop/electron/main.ts::scheduleBackendRespawn`. Без supervisor backend crash = dead app для пользователя; renderer окна виснут "загрузка" forever.
- ❌ Удалять `db_backup::maybe_backup_on_startup` из `main.rs`. Real user data loss — это раз и навсегда; backup это единственный recovery path.
- ❌ Удалять `db::check_integrity` из `init_schema`. Silent corruption хуже чем fail-loud — пользователь не узнает что DB битая, пока данные не разъедутся через sync.

## Sync

- ❌ Direct Rust writer пишет в ARK без вызова `ark_core::db::bump_sync_version_vector`.
- ❌ Добавление нового `Request::Upsert*` / `Request::Delete*` handler'а в `core/ark/crates/ark-core/rust/src/main.rs` без вызова `record_local_upsert` / `record_local_delete`. Раньше legacy handler'ы (UpsertTodo, UpsertProject, UpsertArea, UpsertTag, UpsertHeading, BatchUpsertTodos + Delete\*) тихо пропускали bump → multi-device sync терял локальные правки (2026-05-18 audit). Любой write путь, не записавший в `sync_kv.version_vector`, **не существует** для peers.
- ❌ Batch upsert handler без bump'а `record_local_upsert` per-entity. Один общий bump на батч недостаточен — peer-side sync проверяет HLC entity-id'шно.
- ❌ Ослабление self-peer filtering при изменениях в sync startup.
- ❌ Ослабление routable-address filtering при изменениях в peer persistence.
- ❌ Изменение sync wire-протокола из `snake_case` в что-то другое.
- ❌ Destructive schema migration (`DROP TABLE`, `ALTER COLUMN` несовместимо). Только `CREATE TABLE IF NOT EXISTS` и additive.

### usage-tracker

- ❌ Превращение в Windows Service.
- ❌ Добавление UI / tray icon / окон.
- ❌ Прямой SQL write без `ark_core::db` хелперов и без обновления `version_vector`.
- ❌ Возврат standalone-бинарника по пути services/usage-tracker. После Phase E3 старый usage-tracker больше не active-tree path, а активный код живёт как модуль `platform/runtime/src/usage_tracker/`.
