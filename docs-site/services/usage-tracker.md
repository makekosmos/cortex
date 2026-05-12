# usage-tracker

::: tip Источник правды
`services/usage-tracker/AGENTS.md`, `services/usage-tracker/README.md`
:::

Windows-first фоновый исполняемый файл, который записывает foreground application usage **напрямую в ARK DB**.

## Что записывает

- `tracked_apps`
- `usage_sessions`
- `usage_events`

После каждой группы прямых записей обновляет `lan_sync.version_vector`, чтобы прямые DB writes оставались видимы ARK sync-слою на следующем sync проходе.

## Зачем «напрямую»

Это один из редких исключений правила [«всё через @kepler/ark»](/concepts/write-boundary): tracker — **Rust**, линкуется с `ark_core` как библиотека и использует `ark_core::db` хелперы, которые сами обновляют sync state. Это допустимо.

Но `Arrancador` и любое другое **TS-приложение** должны потреблять usage data **только через ARK**, никогда не запуская собственный tracker и не открывая raw SQLite на запись.

## Намеренно тихий процесс

- Без UI.
- Без tray-иконки.
- Без Windows Service wrapper.
- Один user-level фоновый процесс с маленьким polling loop.

## Структура

```
services/usage-tracker/
├─ src/
│  ├─ main.rs                # активный runtime: polling, session reconciliation, ARK writes
│  └─ windows_capture.rs     # Win32 foreground window sampling, idle detection
├─ installer/
│  ├─ install.ps1            # user-level installer + опциональный autostart
│  └─ uninstall.ps1          # удаление autostart и файлов
└─ scripts/
   └─ build-installer.ps1    # сборка release installer bundle
```

## Defaults

- ARK DB path: `%APPDATA%\Kepler\ark.db`
- Poll interval: `5000` ms
- Idle threshold: `60` s

## Overrides

Через env:

- `ARK_DB_PATH`
- `USAGE_TRACKER_POLL_MS`
- `USAGE_TRACKER_IDLE_SECS`
- `USAGE_TRACKER_RUN_ONCE=1`

Через CLI (перебивают env и defaults):

- `--db-path <path>`
- `--poll-ms <number>`
- `--idle-secs <number>`
- `--once`

## Сборка и тесты

```powershell
cargo test --manifest-path services\usage-tracker\Cargo.toml
bun run --cwd services/usage-tracker build:release    # release exe
bun run --cwd services/usage-tracker package:installer
```

## Installer bundle

`bun run package:installer` создаёт `dist/KeplerUsageTrackerInstaller/` с:

- `usage-tracker.exe`
- `install.ps1`
- `uninstall.ps1`
- `Install Usage Tracker.cmd`
- `Uninstall Usage Tracker.cmd`
- `manifest.json`

И параллельно `dist/KeplerUsageTrackerInstaller.zip`.

Default install target — `%LOCALAPPDATA%\Kepler\UsageTracker`.

Default install behavior:

- Скопировать release-бинарь в install dir.
- Зарегистрировать autostart через `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- Запустить tracker сразу в фоне.

Полезные флаги:

```powershell
install.ps1 -NoStartup
install.ps1 -NoLaunch
install.ps1 -InstallDir D:\Somewhere\UsageTracker
```

## Правила

::: warning Жёстко
- Tracker пишет напрямую в ARK DB и **обязан** обновлять `lan_sync.version_vector` после прямых entity writes.
- Default DB path остаётся `%APPDATA%\Kepler\ark.db`, если не переопределён env или CLI.
- Автоматические тесты и smoke checks **обязаны** переопределить DB path на изолированный `.tmp`, `.e2e`, `.agent/tasks/<TASK_ID>/`, или OS temp. **Не** запускай verification против main user ARK DB.
- Installer — user-level. Не превращай в Windows Service без явного product decision.
- `src/main.rs` — активный runtime path. Дополнительные файлы в `src/` — scaffolding, пока не подключены явно.
:::

## Smoke

```powershell
$env:KEPLER_SMOKE_ROOT = ".agent\tasks\<TASK>\smoke"
$env:ARK_DB_PATH       = "$env:KEPLER_SMOKE_ROOT\usage-tracker\ark.db"
cargo test --manifest-path services\usage-tracker\Cargo.toml
```

## Заметки

- Это обычный user-level фоновый процесс, не Windows Service.
- `Arrancador` потребляет результирующую ARK usage data, не запускает и не владеет процессом.
- `v1` — Windows only. Поздний macOS backend подключается за тем же capture/persistence split.
- `tracked_apps` обновляются на session boundaries; `usage_sessions` и `usage_events` несут fine-grained usage stream.

## Связанные документы

- [Модель данных ARK → usage data](/concepts/ark-objects).
- [Граница записи в ARK](/concepts/write-boundary).
- [Synchronization → Direct writers](/concepts/sync).
- [Arrancador](/apps/arrancador).
