# Background maintenance priority — стабильность ПК при старте

Дата: 2026-06-08
Класс: FULL_LOOP (kepler-backend + ark-core-rpc, performance/stability, native Windows priority APIs, несколько подсистем)
Политика: **AGGRESSIVE_SCOPED_BACKGROUND**

## Проблема

При старте Kosmos `kepler-backend` сатурирует CPU **и I/O** (диск) настолько, что
подвисают USB-устройства / их user-mode сервисы. Воспроизводится и в prod, и в dev.
Это было до правки фокуса лаунчера — отдельный регресс производительности.

Цель пользователя: **стабильность ПК важнее скорости старта.** Допустимо, чтобы
индексация/иконки/бэкап шли медленнее, но они НЕ должны вытеснять систему.

## Корень (по коду)

- `kepler-backend` — `#[tokio::main(worker_threads = 4)]`, процесс NORMAL priority,
  NORMAL I/O priority. Никакого управления приоритетом нигде нет.
- `app_index.rescan()` (`platform/runtime/src/app_index/mod.rs:98`) — `async fn`, но
  внутри **синхронная блокирующая** работа прямо на tokio-воркере (без spawn_blocking):
  - UWP `PackageManager::FindPackages...` (WinRT enum всех пакетов),
  - Start Menu `.lnk` walk,
  - `icons::ensure_icon` — GDI `ExtractIconExW` → PNG encode → запись файла, в тугом
    цикле по всем приложениям. На холодном старте кэш пуст → извлекаются ВСЕ иконки
    пачкой (burst CPU + disk + создание сотен файлов → реакция Defender/индексатора).
    Спавнится на старте через `tokio::spawn` (main.rs:254).
- `file_index.rescan()` — по умолчанию roots пустые (skip), но если пользователь задал
  roots — тяжёлый WalkDir, тоже на старте. Внутренний скан уже в spawn_blocking
  (`file_index/mod.rs:344`), но БЕЗ понижения приоритета потока.
- `db_backup` (`platform/runtime/src/db_backup.rs`) — `maybe_backup_on_startup` на
  startup hot path; реальное копирование внутри ark-core-rpc:
  `Request::DbBackup → with_conn(|conn| db::backup_to_file(conn, dest))`
  (`core/ark/.../main.rs:723`, `db.rs:90`). `with_conn` держит ГЛОБАЛЬНЫЙ DB mutex на
  всё копирование, а `conn.backup(Main, dest, None)` копирует одним шагом → блокирует
  все остальные ARK ops + тяжёлый I/O на normal priority.

## Дизайн

`THREAD_MODE_BACKGROUND_BEGIN/END` (thread-scoped) — роняет CPU **и** I/O приоритет
конкретного потока. Применяем ТОЛЬКО к non-interactive maintenance. RAII-guard.

Жёсткие инварианты:

- ❌ НЕ `PROCESS_MODE_BACKGROUND_BEGIN` на kepler-backend или ark-core-rpc (Microsoft:
  interactive процессы не должны; роняет приоритет всего процесса и новых потоков).
- ❌ НЕ входить в background mode внутри `async fn` с `.await` (task может resume на
  другом потоке pool'а → priority inversion / потерянный END). Guard живёт только
  внутри чистого sync closure / dedicated thread.
- ❌ IPC / WS / launcher search / user-triggered launch — НИКОГДА не background.
- `spawn_blocking` сам по себе не понижает приоритет — поток pool'а остаётся normal.
  Guard должен быть ВНУТРИ sync closure. Для bulk maintenance предпочтителен
  dedicated single worker (concurrency=1), а не общий blocking pool (нет burst).

## Фазы

### Phase 1 — kepler-backend (self-contained, основной выигрыш) — ✅ DONE

1. Новый модуль `platform/runtime/src/priority.rs`:
   - `BackgroundThreadGuard::enter()` → RAII, Windows `SetThreadPriority(GetCurrentThread(),
THREAD_MODE_BACKGROUND_BEGIN)`, Drop → `THREAD_MODE_BACKGROUND_END`. На non-Windows —
     no-op struct.
   - `spawn_background_blocking(f)` — `spawn_blocking` с guard внутри closure.
2. `app_index.rescan()`: вынести discover + icon extraction в sync closure под guard
   (через `spawn_background_blocking`), без `.await` внутри. Async-часть (cache/store
   write) — после, на normal priority.
3. Icon extraction throttle: concurrency=1 (и так серийно) + sleep между иконками
   (`KEPLER_APP_ICON_EXTRACT_SLEEP_MS`, default ~15ms). Только когда реально извлекаем
   (не для cached early-return).
4. Initial app_index rescan: отложить на `KEPLER_APP_INDEX_INITIAL_DELAY_MS` (default
   ~15000) после ready, не на startup hot path.
5. file_index rescan: guard внутри существующего spawn_blocking в scanner.
6. db_backup: убрать со startup hot path — delay `KEPLER_BACKUP_DELAY_MS` (default
   ~120000) после ready (kepler-side; сам chunking — Phase 2).

### Phase 2 — ark-core-rpc backup (более инвазивно, write-boundary/DB-resilience) — ✅ DONE

1. `DbBackup` не должен держать глобальный DB mutex на всю копию: отдельный read-only
   connection к тому же db_path ИЛИ step/chunk backup (`conn.backup` с progress
   callback: N страниц → sleep → N страниц).
2. Поток копирования в ark-core-rpc → `THREAD_MODE_BACKGROUND_BEGIN` (нужно добавить
   `Win32_System_Threading` в windows features ark-core если ещё нет).
3. Обычные ARK requests продолжают обслуживаться во время backup.

### Phase 3 — diagnostics + verification — ✅ DONE (counters lean; WPR = user-run)

1. Diagnostics: `app_index.scan_background_mode` (bool) в `diagnostics.snapshot` —
   подтверждает, что rescan реально шёл на background-priority потоке (AC1/AC3).
   db_backup — log-based (тайминг + `background-priority chunked copy` в success-логе);
   полноценный `background_workers` counters-блок через WsServer::bind признан
   непропорциональным (инвазивный плёнкинг через много call-sites) — намеренно не делал.
2. WPR A/B trace: `wpr-trace.ps1` (CPU + DiskIO + FileIO). Требует admin elevation +
   ручной workload + BEFORE-сборку → запускает пользователь, не агент.

## Acceptance criteria

- AC1: При cold start нет normal-priority app_index/icon/db_backup burst (WPR).
- AC2: app_index initial rescan не стартует раньше backend ready + delay.
- AC3: icon extraction concurrency == 1, с throttle между иконками.
- AC4: db_backup не держит глобальный ARK DB mutex на всю копию (Phase 2).
- AC5: при фоновом скане p95 обычного RPC не растёт драматически.
- AC6: под CPU stress window-move имеет меньше кадров >33ms / >50ms.
- AC7 (cross-platform): non-Windows сборка компилируется (guard = no-op).

## Проверки

- `cargo build -p kepler-backend`
- `cargo test -p kepler-backend` (app_index rescan, priority guard no-op, throttle/delay parsing)
- `cargo test --manifest-path core/ark/crates/ark-core/rust/Cargo.toml` (Phase 2: backup)
- `bun run ark:guard:writes`, `bun run ark:smoke`
- Visual/WPR verify (Phase 3) — честно отметить если не прогнано.
