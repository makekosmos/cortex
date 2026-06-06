# DB resilience и crash safety

::: tip Источник правды

- `platform/runtime/src/db_backup.rs` — periodic backup scheduler
- `platform/runtime/src/crash_reporter.rs` — Rust panic hook
- `core/ark/crates/ark-core/rust/src/db.rs::check_integrity` + `backup_to_file`
- `platform/desktop/electron/main.ts` — Electron crashReporter + supervisor logic
  :::

Локально-первый продукт = вся ответственность за data safety на твоей машине. Эта страница описывает все safety nets, которые работают за кулисами.

## Periodic DB backup

Раз в **24 часа** (override через `KEPLER_BACKUP_INTERVAL_HOURS`) `kepler-backend` на старте делает SQLite Online Backup ARK базы в `%APPDATA%\Kosmos\backups\ark.db.backup-YYYY-MM-DD-HHMMSS`.

**Что значит "Online Backup":**

- Используется `rusqlite::Connection::backup` (`backup` feature) — SQLite C API.
- Source DB остаётся live — concurrent readers и единственный writer не блокируются.
- Это **не** raw `cp` — Online Backup корректно работает с WAL + checkpoints.

**Rotation:**

- После каждого backup'а удаляется всё кроме **последних 7** (override через `KEPLER_BACKUP_RETAIN_COUNT`).
- Sort by parsed timestamp в имени файла (descending) — независимо от mtime.

**Last-run tracking:**

- Timestamp последнего backup'а хранится в `sync_kv` под ключом `kepler.last_backup_ts`.
- При старте, если `now - last_ts < interval` — backup пропускается.
- Если sync_kv ключа нет (свежая установка) — backup запускается сразу.

**Failure modes:**

- Backup failure НЕ блокирует startup backend'а — `eprintln!` в лог, continue.
- User увидит missing backup files в `<data_dir>/backups/`.

## Integrity check на старте

`db::init_schema(conn)` перед applying миграций вызывает:

```rust
conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
```

Если результат != `"ok"` — `init_schema` возвращает `Err(format!("ARK DB integrity check failed: ..."))`.

**Что ловит:**

- WAL/journal corruption после unexpected shutdown.
- Disk corruption (bad sector, file system error).
- Schema drift с corrupted btree pages.

**Что НЕ ловит:**

- Application-level data inconsistencies (например, foreign key violations — те ловятся отдельно через `PRAGMA foreign_keys = ON`).
- Logically wrong data (например, неправильные JSON в `propsJson`).

**После fail-loud:**

- Backend записывает Err в WS init response.
- Kepler shell видит non-connected state → `resetArkClient` → UI показывает error.
- User может попробовать restart, или восстановить из `backups/` вручную.

## Backend supervisor (Electron main)

`platform/desktop/electron/main.ts` — exponential backoff respawn:

| Crash # | Delay перед respawn                    |
| ------- | -------------------------------------- |
| 1       | 1 секунда                              |
| 2       | 5 секунд                               |
| 3       | 30 секунд                              |
| 4       | 60 секунд                              |
| 5       | 120 секунд                             |
| 6+      | Error dialog, automatic restart paused |

**Reset conditions:**

- Если backend проработал **> 5 минут** — counter сбрасывается (это transient crash, не permanent).
- Manual restart через Settings → Перезапустить backend — counter сбрасывается (user action).

**Что считается crash:**

- Exit code != 0 от `backendProc.on('exit')`.
- Code 0 (clean exit, manual stop) — НЕ respawn.

**`isQuiting` guard:**

- При shutdown Kepler shell isQuiting = true — supervisor НЕ respawn'ит backend.

## Crash reporter

### Rust side (kepler-backend)

`crash_reporter::install(data_dir)` в `main()` ставит `std::panic::set_hook`:

```text
<data_dir>/crashes/panic-YYYY-MM-DDTHH-MM-SS.SSSZ.log
```

Файл содержит:

- `kepler-backend v0.1.X` (CARGO_PKG_VERSION)
- timestamp (RFC 3339)
- panic message (downcast `&str` / `String`)
- location (`file:line:column`)
- `std::backtrace::Backtrace::capture()` — full Rust backtrace

**Требование:** `RUST_BACKTRACE=1` env. Kepler shell сетит при `spawn`е backend'а — production users всегда получают full backtrace.

### Electron side (main + renderer)

`crashReporter.start({ productName: "Kepler", uploadToServer: false })` — собирает minidump'ы Electron main и renderer crashes в `app.getPath("crashDumps")` (по умолчанию `%APPDATA%\Kepler\Crashpad\`).

**Локальные only:** `uploadToServer: false` — никаких отправок third-party.

### Settings UI

Settings → General → секция "Отчёты об ошибках":

- Hint показывает count crash files в папке.
- Кнопка **«Открыть папку»** — `shell.openPath(<data_dir>/crashes/)`.
- Кнопка **«Очистить»** — удаляет все crash файлы.

После 5 crashes подряд — backend supervisor показывает error dialog со ссылкой на эту папку. User может прислать crash log developer'у для диагностики.

## Mutex poison recovery

В `core/ark/crates/ark-core/rust/src/db.rs::SqliteStorageBackend` (sync runtime):

```rust
let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
```

**Зачем:** в Rust `std::sync::Mutex` если thread panic'ует при удержании lock'а, mutex становится "poisoned" — `lock()` возвращает `Err(PoisonError)`. По умолчанию `unwrap()` propagate'ит panic дальше → каждый последующий call падает forever, до process restart.

**Recovery:** `unwrap_or_else(|e| e.into_inner())` извлекает inner guard. SQLite transactions atomic — partially-applied state невозможен, данные внутри guard'а валидны.

**Применяется к:** `load_entities`, `apply_entity`, `get_kv`, `set_kv`, `set_device_id`, `device_id` — все методы `StorageBackend` trait + private device_id state.

## Property-based tests

`core/ark/crates/ark-core/rust/tests/proptest_invariants.rs` — random Upsert/Delete sequences проверяют invariants:

1. **List survives random ops** — никакая sequence не panic'ает list_objects().
2. **HLC monotonic per entity** — version_vector HLC строго растёт после каждой write op.
3. **Tombstone lifecycle consistent** — после delete есть tombstone, после re-insert исчез.

Run: `cargo test --manifest-path core/ark/crates/ark-core/rust/Cargo.toml --test proptest_invariants`.

## Что НЕ покрывают эти safety nets

- **Cross-device cloud backup.** Если ноут сгорел, backup'ы локальные тоже сгорели. Cloud sync — отдельный roadmap item.
- **DB encryption at rest.** ARK DB plaintext SQLite. Если нужен encrypted-at-rest — SQLCipher feature (отдельная задача).
- **Logical bugs.** Crash reporter ловит panic'и; logical data corruption (например, неправильный sync merge) — нет.
- **Network FS под `%APPDATA%`.** Все Kepler safety nets (singleton lock, ARK WAL, backups) предполагают **локальную** файловую систему. Если `%APPDATA%` roaming-профиль через SMB/NFS — `SingletonGuard` (SQLite WAL `BEGIN IMMEDIATE`) и сам ARK ведут себя непредсказуемо: WAL поверх сетевой ФС официально не поддерживается SQLite, mandatory locks на SMB реализованы серверо-специфично. Симптомы — fantom singleton conflicts, stale reads, или corruption на disconnect. Не чиним: roaming-профили для Kepler out-of-scope, документируем как известное ограничение.

## Restore from backup (manual)

Если нужно восстановить ARK из backup:

1. Закрыть Kepler полностью (Quit через tray).
2. Скопировать `%APPDATA%\Kosmos\backups\ark.db.backup-<TIMESTAMP>` → `%APPDATA%\Kosmos\ark.db` (заменив current).
3. Удалить `%APPDATA%\Kosmos\ark.db-wal` и `ark.db-shm` (если есть) — WAL может конфликтовать с восстановленной DB.
4. Запустить Kepler. `PRAGMA integrity_check` на старте проверит восстановленную DB; в случае проблем — fail loud.

Автоматизированный restore UI — отдельный roadmap item.
