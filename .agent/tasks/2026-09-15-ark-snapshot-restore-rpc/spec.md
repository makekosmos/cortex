# Spec — KOS-51: atomic ARK snapshot restore RPC в Core

Linear: https://linear.app/yosokosmos/issue/KOS-51/core-dobavit-atomarnyj-ark-snapshot-restore-rpc

## Цель

Добавить в `ark-core-rpc` privileged RPC для list/validate/restore ARK DB
snapshot'ов из Core-owned `backups/` директории. Restore применяется внутри
владельца live `Connection` через SQLite Online Backup API — нет момента,
когда primary DB отсутствует или частично заменён файлом. Блокер для
Manager restore UI (KOS-27).

## Контекст

- `ark-core-rpc` владеет единственным live `Connection`
  (`DB: StdMutex<Option<Arc<StdMutex<Connection>>>>`), путём (`DB_PATH`) и
  serial RPC-loop'ом. `Request::DbBackup` копирует DB на фоновом потоке через
  ОТДЕЛЬНЫЙ read-коннекшн (`db::backup_to_file_chunked`), не держа глобальный
  mutex.
- Backups пишутся вызывающей стороной (Cortex/kepler-backend) в
  `<dir(ark.db)>/backups/ark.db.backup-*`. Restore принимает только basename
  id файла из этой директории — никаких произвольных путей от caller'а.
- Попытки делать file-swap `ark.db` снаружи (остановить child, заменить
  файл) — out of scope: crash window + нет общего lock'а + TOCTOU.

## Скоуп

В задаче:

- `ark_core::db::snapshot` модуль: `list_snapshots`, `validate_snapshot`,
  `restore_snapshot` + no-follow open источника + нормализованный schema
  fingerprint.
- RPC `db_backup_list`, `db_backup_validate { backup_id }`,
  `db_backup_restore { backup_id }`.
- Сериализация `db_backup` (фоновый поток) и `db_backup_restore` одним
  `BACKUP_GATE` mutex'ом.
- Pre-restore rollback snapshot + автоматический откат при провале
  post-restore verification.
- Тесты: happy path, restart-equivalent reopen, traversal/id rejection,
  fake/corrupt/foreign-schema snapshot, backup↔restore serialization,
  rollback-on-verify-fail.

Вне задачи:

- Cortex/Manager UI (KOS-27) — отдельный репозиторий, отдельная задача.
- Изменение wire-формата `db_backup` или ротация backups.
- Cloud/remote restore, partial restore, restore в другой db path.

## Дизайн (заморожено)

- `backups_dir = parent(DB_PATH)/backups`. `backup_id` обязан быть basename:
  один `Component::Normal`, без разделителей/`..`/`:`.
- Источник открывается no-follow: `symlink_metadata` reject non-regular +
  на Windows `FILE_FLAG_OPEN_REPARSE_POINT` + post-open `file_type()` check.
  Байты копируются в staging-файл внутри `backups/`; вся SQLite-валидация и
  restore идут с staging копии — исходный snapshot не модифицируется и нет
  TOCTOU на его содержимое.
- Validation: `PRAGMA integrity_check` на staging + сравнение нормализованного
  `sqlite_schema` fingerprint'а (type/name/tbl_name/normalized sql +
  `user_version` + `application_id` + содержимое `canonical_migration_runs`)
  со snapshot'ом live DB.
- Apply: `conn.restore(Main, staging)` под глобальным DB mutex (restore
  держит `BACKUP_GATE` → conn mutex). Online Backup пишет destination
  транзакционно — crash в середине оставляет pre-restore состояние.
- Rollback: перед apply `conn.backup(Main, rollback-staging)` в `backups/`;
  при провале post-verify (`integrity_check` + schema fingerprint) —
  `conn.restore` из rollback файла; typed error наружу.
- `db_backup` thread захватывает `BACKUP_GATE` на всё копирование — backup и
  restore не пересекаются.
- Typed result `db_backup_restore`: `{ restored: true, id, objects, links }` —
  Cortex по нему делает reload. `db_backup_validate`:
  `{ id, exists, integrity_ok, schema_match, valid, error }`.
  `db_backup_list`: `{ backups: [{ id, size_bytes, modified_ms }] }`.

## Acceptance Criteria

- **AC1.** `db_backup_restore` применяет snapshot атомарно: после ответа
  `restored: true` live DB содержит объекты snapshot'а; в процессе нет
  момента отсутствующего/частично заменённого primary DB (нет rename/unlink
  live файла; destination пишется транзакционно Online Backup API).
- **AC2.** `db_backup` и `db_backup_restore` сериализованы `BACKUP_GATE`:
  restore, запущенный во время активного backup, выполняется только после
  завершения копирования (тест доказывает ordering).
- **AC3.** Невалидный (`backup_id` с traversal/не basename), отсутствующий,
  не-SQLite, частично повреждённый или чужой-schema snapshot не меняет live
  DB: запрос возвращает typed error/`valid:false`, содержимое live DB
  побайтово-эквивалентно по `objects`/`object_types`/`sync_kv`.
- **AC4.** После drop+reopen connection (restart-equivalent) из live DB
  читаются те же ARK objects/object_types, что в snapshot.
- **AC5.** Провал post-restore verification откатывает live DB из
  pre-restore rollback snapshot'а (тест через injected verifier seam).
- **AC6.** `db_backup_list` возвращает только regular files из
  `backups/` с `id` = basename; `db_backup_validate` даёт per-check verdict.
- **AC7.** `cargo test --manifest-path crates/ark-core/Cargo.toml` и
  `cargo clippy` чисты (workspace lints: `unwrap_used` deny в prod code).
