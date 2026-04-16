# Electron Audit

Date: 2026-04-16
Project: Arrancador
Scope: Electron app shell, renderer, IPC, activity watcher, scan/launch flows, network/RAWG surface, legacy `src-tauri` subtree

## Context

Этот файл фиксирует текущее состояние аудита перед переносом проекта внутрь другого репозитория.

Аудит был собран из:

- локального ручного прохода по критичным узлам
- параллельного обзора 10 срезов через субагентов
- проверки кода Electron, renderer, activity watcher и legacy `src-tauri`

Ограничение среды: одновременно было доступно только 6 агентных потоков, поэтому аудит шёл в две волны.

## High Severity

### 1. Слишком широкий Electron attack surface

- У окна включён `sandbox: false`
- preload публикует общий `invoke/on` bridge без allowlist
- в `index.html` нет CSP
- renderer может вызывать `shell_open_path` и `shell_open_external` без жёсткой валидации назначения

Риск:

- любой XSS в renderer быстро превращается в IPC-to-OS эскалацию
- внешний URL/open-path surface сейчас доверяет данным сильнее, чем должен

Ссылки:

- [electron/main/windows.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/windows.ts:91)
- [electron/preload.ts](D:/Personal/Hobby/Coding/arrancador/electron/preload.ts:4)
- [electron/shared/ipc.ts](D:/Personal/Hobby/Coding/arrancador/electron/shared/ipc.ts:10)
- [electron/main/backend.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/backend.ts:690)
- [index.html](D:/Personal/Hobby/Coding/arrancador/index.html:1)

### 2. Tray/startup lifecycle сломан по parity и по устойчивости

- `start_minimized_in_tray` фактически не отрабатывает как пользователь ожидает
- окно при старте принудительно показывается
- close/minimize могут спрятать приложение ещё до гарантированного создания tray
- preload/renderer crash не считается фатальным и оставляет живой broken process с занятым single-instance lock

Риск:

- пользователь получает скрытое или сломанное приложение без нормального recovery path
- поведение отличается от ожидаемого tray/minimize сценария

Ссылки:

- [electron/main/index.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/index.ts:73)
- [electron/main/index.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/index.ts:175)
- [electron/main/windows.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/windows.ts:102)
- [electron/main/windows.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/windows.ts:123)
- [electron/main/windows.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/windows.ts:143)

### 3. Scan/launch flow не согласован с поддержкой `.lnk`

- folder scan видит только `.exe`
- `launchGame()` запускает `exe_path` напрямую
- shortcut resolution не встроен в основной launch path

Риск:

- shortcut-based игры частично поддерживаются в UI, но не поддерживаются как полноценный runtime сценарий

Ссылки:

- [electron/main/services/helpers/scan.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/services/helpers/scan.ts:8)
- [electron/main/services/scan.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/services/scan.ts:14)
- [electron/main/services/games.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/services/games.ts:460)
- [electron/main/services/games/process.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/services/games/process.ts:200)

### 4. `GameDetail` теряет пользовательские черновики и подвержен race condition

- refresh shared game list перетирает локальный draft state
- загрузка backup list не отменяется при быстрой смене игры
- поздний ответ может записать бэкапы не той игры в текущий экран

Риск:

- потеря несохранённых правок
- UI показывает данные от другой игры

Ссылки:

- [src/pages/GameDetail.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/GameDetail.tsx:230)
- [src/pages/GameDetail.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/GameDetail.tsx:253)
- [src/pages/GameDetail.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/GameDetail.tsx:291)
- [src/pages/GameDetail.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/GameDetail.tsx:468)

### 5. Library вызывает install-status IPC fan-out по всей коллекции

- при любом изменении `games` происходит `Promise.all(games.map(isInstalled))`
- это триггерится не только на path changes, но и на рейтинг, заметки, playtime, favorite и другие обновления
- при ошибках fallback идёт в `true`

Риск:

- деградация производительности на большой библиотеке
- install-state фильтр может врать

Ссылки:

- [src/pages/Library.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/Library.tsx:543)
- [src/pages/Library.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/Library.tsx:733)

### 6. RAWG/URL trust boundary небезопасен

- любые `http(s)` передаются в `shell.openExternal`
- удалённые `background_image` сохраняются и потом рендерятся как доверенные `img src`
- нет host/scheme allowlist
- нет `referrerPolicy`

Риск:

- privacy leak
- лишняя внешняя поверхность для вредоносных или неожиданных URL

Ссылки:

- [src/lib/browser.ts](D:/Personal/Hobby/Coding/arrancador/src/lib/browser.ts:292)
- [electron/main/backend.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/backend.ts:691)
- [src/components/RawgMetadataPrompt.tsx](D:/Personal/Hobby/Coding/arrancador/src/components/RawgMetadataPrompt.tsx:311)
- [src/pages/Catalogue.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/Catalogue.tsx:245)
- [src/pages/GameDetail.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/GameDetail.tsx:783)

### 7. Activity watcher ещё не достаточно надёжен как отдельный модуль

- process matching идёт по raw path equality
- нет полноценной нормализации alias/symlink/junction путей
- watcher и Electron main конкурируют за один SQLite-файл с узким запасом по locking

Риск:

- часть игр не будет трекаться
- возможны `SQLITE_BUSY` / `LOCKED` и потеря апдейтов

Ссылки:

- [activity-watcher/src/watcher.rs](D:/Personal/Hobby/Coding/arrancador/activity-watcher/src/watcher.rs:87)
- [activity-watcher/src/watcher.rs](D:/Personal/Hobby/Coding/arrancador/activity-watcher/src/watcher.rs:137)
- [activity-watcher/src/watcher.rs](D:/Personal/Hobby/Coding/arrancador/activity-watcher/src/watcher.rs:210)
- [activity-watcher/src/db.rs](D:/Personal/Hobby/Coding/arrancador/activity-watcher/src/db.rs:27)
- [electron/main/db/sqlite.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/db/sqlite.ts:27)

## Medium Severity

### 8. Dev bootstrap остаётся хрупким

- переиспользуется любой HTTP `200` на `127.0.0.1:5173`
- Electron может стартовать на stale bundles

Риск:

- ложные blank screen / wrong app / stale renderer сценарии в dev

Ссылки:

- [scripts/dev.ts](D:/Personal/Hobby/Coding/arrancador/scripts/dev.ts:75)
- [scripts/dev.ts](D:/Personal/Hobby/Coding/arrancador/scripts/dev.ts:84)
- [scripts/dev.ts](D:/Personal/Hobby/Coding/arrancador/scripts/dev.ts:126)

### 9. Scan/import и Spotlight плохо масштабируются

- scan пушит IPC и `setState` по одному entry
- drag-drop import делает последовательные IPC-проверки
- в layout постоянно смонтированы две копии `Spotlight`

Риск:

- лишняя нагрузка на renderer и IPC
- деградация UX на больших библиотеках и больших scan folders

Ссылки:

- [src/pages/Scan.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/Scan.tsx:247)
- [src/pages/Scan.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/Scan.tsx:283)
- [src/pages/Layout.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/Layout.tsx:37)
- [src/pages/Layout.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/Layout.tsx:79)
- [src/components/Spotlight.tsx](D:/Personal/Hobby/Coding/arrancador/src/components/Spotlight.tsx:93)

### 10. Activity watcher launcher всё ещё слишком привязан к layout текущего repo

- Electron-side launcher ищет бинарь по длинному списку repo-specific путей и legacy имён

Риск:

- дальнейшее извлечение watcher-а в отдельный репозиторий будет всё ещё требовать ручной чистки интеграционного слоя

Ссылки:

- [electron/main/activity-watcher.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/activity-watcher.ts:96)

### 11. Path dedupe и process matching местами слишком наивны

- dedupe по `exe_path` основан на raw string equality
- POSIX process matching ломается на путях с пробелами
- kill path считает отправленные сигналы, а не подтверждённые завершения

Ссылки:

- [electron/main/services/games.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/services/games.ts:432)
- [electron/main/services/helpers/process.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/services/helpers/process.ts:25)
- [electron/main/services/helpers/process.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/services/helpers/process.ts:150)

### 12. RAWG input/persistence hygiene слабая

- input validation неполная
- remote metadata сохраняется как есть
- malformed API key сохраняется без нормализации

Ссылки:

- [electron/main/helpers/rawg.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/helpers/rawg.ts:75)
- [electron/main/services/metadata.ts](D:/Personal/Hobby/Coding/arrancador/electron/main/services/metadata.ts:46)
- [src/hooks/useSettingsState.ts](D:/Personal/Hobby/Coding/arrancador/src/hooks/useSettingsState.ts:78)

## Low Severity

### 13. Catalogue и artwork loading не оптимизированы

- catalogue grid грузит remote images без `loading="lazy"` и `decoding="async"`

Ссылка:

- [src/pages/Catalogue.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/Catalogue.tsx:245)

### 14. `SystemInfo` не запрашивает fresh data при первом открытии

- страница гидратируется из cache и ждёт ручного refresh

Ссылка:

- [src/pages/SystemInfo.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/SystemInfo.tsx:99)

### 15. `running processes` view устаревает после refresh

- usage update не добавляет новые процессы и не удаляет завершившиеся

Ссылка:

- [src/pages/Scan.tsx](D:/Personal/Hobby/Coding/arrancador/src/pages/Scan.tsx:83)

## Legacy `src-tauri` Note

Если legacy `src-tauri` subtree остаётся в проекте как активная ветка кода, там тоже есть серьёзные проблемы:

- DB startup нефатальный
- schema init неатомарный
- backup DB/filesystem cleanup неатомарный
- часть backup flows может оставлять inconsistent state

Ссылки:

- [src-tauri/src/lib.rs](D:/Personal/Hobby/Coding/arrancador/src-tauri/src/lib.rs:113)
- [src-tauri/src/database.rs](D:/Personal/Hobby/Coding/arrancador/src-tauri/src/database.rs:44)
- [src-tauri/src/backup.rs](D:/Personal/Hobby/Coding/arrancador/src-tauri/src/backup.rs:1164)
- [src-tauri/src/backup.rs](D:/Personal/Hobby/Coding/arrancador/src-tauri/src/backup.rs:1295)

## Agent Coverage

Полные findings успели вернуть эти срезы:

- renderer performance and logic
- Electron lifecycle and shell
- SQLite/data layer
- activity watcher and extraction-readiness
- scan/launch/process flows
- RAWG/network surface

Остальные срезы были запущены, но остановлены по таймауту без финального набора findings:

- backup/restore dedicated pass
- IPC bridge dedicated pass
- packaging/release surface pass
- overall user-facing regression pass

## Suggested Next Step After Migration

После переноса проекта в другой репозиторий имеет смысл первым делом собрать remediation backlog в таком порядке:

1. P0: Electron security surface, tray/startup lifecycle, `.lnk` launch parity, watcher DB contention
2. P1: Library/GameDetail race conditions, install-status IPC storm, scan/import scaling
3. P2: RAWG hygiene, lazy loading, stale pages, extraction cleanup для watcher integration
