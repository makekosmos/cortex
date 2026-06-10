# usage-tracker

::: tip Статус
После Phase E3 (2026-05-14) `usage-tracker` **больше не standalone-сервис**. Активный код живёт как модуль в составе kepler-backend — `platform/runtime/src/usage_tracker/`. Старый бинарь (исторический путь services/usage-tracker) — frozen legacy и больше не active-tree path.
:::

Записывает process runtime/playtime и foreground activity **напрямую в ARK DB** изнутри kepler-backend процесса. На устройстве пользователя нет отдельного `usage-tracker.exe` — модуль стартует автоматически вместе с backend'ом (Phase E2 wiring).

## Что записывает

- `tracked_apps`
- `usage_sessions` — `runtime_ms` считает время процесса после первого наблюдения только пока у него есть видимое не-свёрнутое top-level окно; `foreground_ms` — только активное foreground-окно, `idle_ms` — foreground-idle.
- `usage_events`

После каждой группы прямых записей обновляет `lan_sync.version_vector`, чтобы прямые DB writes оставались видимы ARK sync-слою на следующем sync проходе.

## Зачем «напрямую»

Это одно из редких исключений правила [«всё через @kosmos/ark»](/concepts/write-boundary): tracker — **Rust**, линкуется с `ark_core` как библиотека и использует `ark_core::db` хелперы, которые сами обновляют sync state. Это допустимо.

`Arrancador` и любое другое **TS-приложение / extension** должны потреблять usage data **только через ARK** (`@kosmos/ark` SDK), никогда не запуская собственный tracker и не открывая raw SQLite на запись.

## Структура модуля

```
platform/runtime/
└─ src/
   └─ usage_tracker/
      ├─ mod.rs                # активный runtime: polling, process sessions, ARK writes
      └─ windows_capture.rs    # Win32 foreground sampling, visible-window checks, idle detection
```

Подключён в `platform/runtime/src/lib.rs` и стартует из `platform/runtime/src/main.rs` после того как backend подключился к `ark-core-rpc`.

## Defaults

- ARK DB path: `%APPDATA%\Kosmos\ark.db` (либо `KOSMOS_DB_PATH`, если backend получил его от `platform/desktop/electron/main.ts`).
- Poll interval: `1000` ms
- Idle threshold: `60` s

## Overrides

Через env (читает kepler-backend):

- `ARK_DB_PATH` / `KOSMOS_DB_PATH`
- `USAGE_TRACKER_POLL_MS`
- `USAGE_TRACKER_IDLE_SECS`

## Сборка и тесты

Модуль собирается как часть `platform/runtime`:

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

Standalone бинарь `usage-tracker.exe` со собственным installer'ом (`install.ps1` / `uninstall.ps1`, autostart через `HKCU\...\Run`) заморожен. Возвращать его в активный код запрещено (см. [Запреты](/agents/forbidden#usage-tracker)).

## Заметки

- Tracker остаётся user-level — внутри backend процесса, который spawn'ит Kepler shell.
- `Arrancador` extension потребляет результирующую ARK usage data через `@kosmos/ark`, не запускает и не владеет процессом.
- Windows-only capture. Поздний macOS backend подключается за тем же capture/persistence split.
- `tracked_apps` обновляются при старте и foreground-сэмплах; `usage_sessions` сохраняются каждый poll tick, чтобы crash/backend restart терял максимум последний tick.
- Game playtime и Dashboard “Суммарное время” используют `runtime_ms`; `foreground_ms` остаётся диагностикой “активно на экране”.
- Свёрнутая игра не накручивает runtime. Overlay-сценарии вроде Discord/Steam продолжают считаться, пока окно игры остаётся visible/non-iconic под overlay.

## Связанные документы

- [Модель данных ARK → usage data](/concepts/ark-objects).
- [Граница записи в ARK](/concepts/write-boundary).
- [Synchronization → Direct writers](/concepts/sync).
- [Arrancador](/apps/arrancador).
