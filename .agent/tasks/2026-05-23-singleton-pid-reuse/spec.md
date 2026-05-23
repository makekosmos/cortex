# 2026-05-23 — kepler-backend: stale lock-file гейт ломается на pid reuse

## Контекст

`services/kepler-backend/src/main.rs:182-191` гейтит startup на
`lock_file::read_if_alive(kepler.lock.json)`. Эта функция проверяет «жив ли
PID, записанный в JSON» через `OpenProcess` (Win) / `kill(pid, 0)` (Unix).

Если предыдущий kepler-backend упал без cleanup'а (panic, kill -9, BSOD,
выдернутый шнур), JSON остаётся на диске с записанным PID. Windows
переиспользует освободившийся PID для любого следующего процесса —
типично через несколько часов это будет electron-shell, который сам же
запускает новый kepler-backend. Гейт видит «PID жив» → возвращает
`SingletonError("singleton conflict via lock-file")` → backend никогда
не стартует, пока пользователь руками не удалит JSON.

Наблюдалось 2026-05-23: pid 14852 в lock-файле (написан 17:56 предыдущим
backend'ом) к 23:05 принадлежал текущему electron.exe. Supervisor ушёл в
бесконечный respawn loop, шелл показывал `ArkClient not ready (timeout)`.

Полный root cause + observability в `docs-site/agents/postmortems.md`
§ 2026-05-23 — Kepler: singleton conflict из-за pid reuse.

## Решение

Убрать pid-based гейт. Доверять `SingletonGuard::acquire` (SQLite WAL
`BEGIN IMMEDIATE` на `kepler-singleton.lock.db`) — это OS-level file
lock, kernel освобождает handle при любой смерти процесса. Pid reuse
физически не может его сломать.

`kepler.lock.json` остаётся discovery-метаданными для shell (ws_port,
auth_token). После успешного `acquire` сразу удаляем stale JSON (закрыть
окно «старый ws_port на диске пока новый WS ещё не bind'нулся»),
дальше `write_atomic` после `ws.bind` пишет новый.

## Файлы

- `services/kepler-backend/src/singleton.rs` — pub helper
  `acquire_clearing_stale_lock(lock_path, singleton_path)` объединяющий
  acquire и удаление stale JSON. Возвращает stale_pid для диагностики.
- `services/kepler-backend/src/main.rs:180-191` — заменить inline-гейт на
  вызов helper'а.
- `services/kepler-backend/src/lock_file.rs` — удалить `read_if_alive`,
  `is_pid_alive` и их тесты (мёртвый код).
- `docs-site/agents/postmortems.md` — entry с root cause + Fix + Prevention.

## Acceptance Criteria

- **AC1.** `acquire_clearing_stale_lock` возвращает `Ok((SingletonGuard, Some(pid)))`
  если на диске лежит lock-файл с PID живого процесса, не являющегося kepler-backend.
  Кейс воспроизводит pid reuse без необходимости спавнить процесс.
- **AC2.** `acquire_clearing_stale_lock` после `Ok(_)` гарантирует, что
  `lock_path` удалён с диска. Следующая попытка `read(lock_path)` возвращает
  `NotFound`.
- **AC3.** Параллельный второй вызов `acquire_clearing_stale_lock` на тот же
  `singleton_path` возвращает `Err(SingletonError::AlreadyRunning)`. Покрыто
  существующим `second_acquire_fails_fast` + добавляем тест на
  `acquire_clearing_stale_lock` напрямую.
- **AC4.** `lock_file::read_if_alive` и `is_pid_alive` удалены вместе с
  тестами `read_if_alive_returns_some_for_current_process`,
  `read_if_alive_returns_none_for_dead_pid`,
  `read_if_alive_returns_none_when_file_missing`. Нет references из других
  модулей (grep clean).
- **AC5.** `main.rs::setup()` больше не возвращает `Err("singleton conflict
via lock-file")`. Единственный путь отказа на singleton — `SingletonError::
AlreadyRunning` от настоящего OS-lock'а.
- **AC6.** `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`
  зелёный.
- **AC7.** Manual repro: запустить `bun run --cwd shell dev`, дождаться bind,
  убить kepler-backend через `Stop-Process` (без cleanup), не трогать
  electron, дождаться respawn — backend должен подняться (старый JSON
  игнорируется, не блокирует).
- **AC8.** Postmortem entry в `docs-site/agents/postmortems.md` заполнен
  целиком (Симптомы / Где жило / Root cause / Fix / Регрешн-защита /
  Prevention). Запись сверху, под template'ом.

## Запреты

- Не трогать ACL hardening (`apply_owner_only_permissions`), `write_atomic`,
  `KeplerLockFile` shape — не относится к багу.
- Не менять `SingletonGuard` API. Только добавить новый pub helper рядом.
- Не упрощать error reporting — сохранить `tracing::info!` с stale_pid для
  диагностики (это первое что нужно увидеть в логах если бы был follow-up
  инцидент).
- Не трогать сторонние lock-механизмы (`services/kepler-focus-svc/`,
  `services/usage-tracker/` — отдельные scope'ы).
