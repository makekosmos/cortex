# usage-tracker

::: tip Статус
После Phase E3 (2026-05-14) `usage-tracker` **больше не standalone-сервис**. Активный код живёт как модуль в составе kepler-backend — `services/kepler-backend/src/usage_tracker/`. Старый бинарь (исторический путь services/usage-tracker) заморожен в `legacy/usage-tracker/`.
:::

Записывает foreground application usage **напрямую в ARK DB** изнутри kepler-backend процесса. На устройстве пользователя нет отдельного `usage-tracker.exe` — модуль стартует автоматически вместе с backend'ом (Phase E2 wiring).

## Что записывает

- `tracked_apps`
- `usage_sessions`
- `usage_events`

После каждой группы прямых записей обновляет `lan_sync.version_vector`, чтобы прямые DB writes оставались видимы ARK sync-слою на следующем sync проходе.

## Зачем «напрямую»

Это одно из редких исключений правила [«всё через @kepler/ark»](/concepts/write-boundary): tracker — **Rust**, линкуется с `ark_core` как библиотека и использует `ark_core::db` хелперы, которые сами обновляют sync state. Это допустимо.

`Arrancador` и любое другое **TS-приложение / extension** должны потреблять usage data **только через ARK** (`@kepler/ark` SDK), никогда не запуская собственный tracker и не открывая raw SQLite на запись.

## Структура модуля

```
services/kepler-backend/
└─ src/
   └─ usage_tracker/
      ├─ mod.rs                # активный runtime: polling, session reconciliation, ARK writes
      └─ windows_capture.rs    # Win32 foreground window sampling, idle detection
```

Подключён в `services/kepler-backend/src/lib.rs` и стартует из `services/kepler-backend/src/main.rs` после того как backend подключился к `ark-core-rpc`.

## Defaults

- ARK DB path: `%APPDATA%\Kosmos\ark.db` (либо `KOSMOS_DB_PATH`, если backend получил его от `shell/electron/main.ts`).
- Poll interval: `5000` ms
- Idle threshold: `60` s

## Overrides

Через env (читает kepler-backend):

- `ARK_DB_PATH` / `KOSMOS_DB_PATH`
- `USAGE_TRACKER_POLL_MS`
- `USAGE_TRACKER_IDLE_SECS`
- `USAGE_TRACKER_RUN_ONCE=1`

## Сборка и тесты

Модуль собирается как часть `services/kepler-backend`:

```powershell
cargo build --manifest-path services\kepler-backend\Cargo.toml --bin kepler-backend
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib
```

## Правила

::: warning Жёстко
- Модуль пишет напрямую в ARK DB и **обязан** обновлять `lan_sync.version_vector` после прямых entity writes.
- Default DB path остаётся `%APPDATA%\Kosmos\ark.db`, если не переопределён env.
- Автоматические тесты и smoke checks **обязаны** переопределить DB path на изолированный `.tmp`, `.e2e`, `.agent/tasks/<TASK_ID>/`, или OS temp. **Не** запускай verification против main user ARK DB.
- Не превращай в Windows Service.
- Не добавляй UI, tray, окна — capture модуль остаётся пассивным.
:::

## Smoke

```powershell
$env:KOSMOS_SMOKE_ROOT = ".agent\tasks\<TASK>\smoke"
$env:ARK_DB_PATH       = "$env:KOSMOS_SMOKE_ROOT\usage-tracker\ark.db"
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib
```

## Legacy standalone

Standalone бинарь `usage-tracker.exe` со собственным installer'ом (`install.ps1` / `uninstall.ps1`, autostart через `HKCU\...\Run`) заморожен. Если нужен такой режим обратно — есть архив в `legacy/usage-tracker/`, но возвращать его в активный код запрещено (см. [Запреты](/agents/forbidden#usage-tracker)).

## Заметки

- Tracker остаётся user-level — внутри backend процесса, который spawn'ит Kepler shell.
- `Arrancador` extension потребляет результирующую ARK usage data через `@kepler/ark`, не запускает и не владеет процессом.
- Windows-only capture. Поздний macOS backend подключается за тем же capture/persistence split.
- `tracked_apps` обновляются на session boundaries; `usage_sessions` и `usage_events` несут fine-grained usage stream.

## Связанные документы

- [Модель данных ARK → usage data](/concepts/ark-objects).
- [Граница записи в ARK](/concepts/write-boundary).
- [Synchronization → Direct writers](/concepts/sync).
- [Arrancador](/apps/arrancador).
