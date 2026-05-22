---
title: App Index
description: Как Kepler хранит и индексирует установленные приложения для поисковика — Windows v1 с cross-platform trait.
---

# App Index

Модуль `services/kepler-backend/src/app_index/` — поиск и запуск установленных приложений из лаунчера Kepler. Cross-platform-ready: общий trait + per-platform impl. Windows реализован, macOS и Linux — позже.

## Зачем отдельная база (не ARK)

App-индекс **host-specific** и **regenerable**. Хранить его в ARK как `app_obj` — плохо по трём причинам:

1. **Пути зависят от машины.** `C:\Users\<user>\AppData\Local\Discord\Update.exe` не имеет смысла на другом устройстве. Синхронизация ничего не даст полезного, только конфликты.
2. **Sync-журнал.** Каждое событие fs-watcher'а (установка/удаление приложения) bump'ило бы `sync_version_vector`, генерировало бы дельты для пуша на peer'ы — это шум.
3. **Регенерация дешёвая.** Полный rescan Windows-машины — ~300мс. Сохранять снапшот не нужно для целостности, можно всегда пересчитать.

Соответствует духу [write-boundary](./write-boundary): ARK = пользовательские данные, app-index = производное состояние хоста.

**Frecency** (как часто и недавно ты запускаешь приложение) — другая история. Это уже user behaviour data, имеет смысл синхронизировать. Frecency остаётся в ARK как `usage_event_obj` (уже существующий объектный тип). Join делается в памяти при `app_index.search`.

## Хранилище

```
<instance.dataDir>/
  ark.db              ← ARK objects + sync
  app-index.db        ← App Index (отдельная SQLite, WAL)
  app-icons/
    <sha256(exec_path)[..16]>.png  ← извлечённые иконки
```

Schema (`apps` таблица):

```sql
CREATE TABLE apps (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL,
  exec_path       TEXT NOT NULL,
  icon_path       TEXT,
  kind            TEXT NOT NULL,   -- 'win32' | 'uwp' | 'mac_bundle' | 'linux_desktop'
  source          TEXT NOT NULL,   -- 'start_menu' | 'uwp' | ...
  mtime           INTEGER NOT NULL,
  last_indexed_at INTEGER NOT NULL
);
CREATE INDEX apps_name_ci ON apps(name COLLATE NOCASE);
CREATE INDEX apps_source  ON apps(source);
```

Все per-instance — slot-аware через [instance.ts](./instances).

## Архитектура

```
AppIndex (singleton, Arc-shared)
  ├─ sources: Vec<Box<dyn AppSource>>
  │   ├─ StartMenuSource (Windows)
  │   ├─ UwpSource (Windows)
  │   └─ ... позже: SpotlightSource (macOS), XdgDesktopSource (Linux)
  ├─ store: AppStore (SQLite app-index.db)
  ├─ cache: Arc<RwLock<Vec<App>>>  ← hot path для search
  └─ icon_cache_dir
```

Trait:

```rust
pub trait AppSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn discover(&self) -> Result<Vec<App>>;
}
```

Один платформенный модуль регистрирует несколько источников (Windows: Start Menu + UWP). Внутри один источник может работать с несколькими типами артефактов.

## Cold-start

При старте backend'а:

1. Открыть `app-index.db`, прочитать cached apps в `Arc<RwLock<Vec<App>>>` — sync, **<10 мс**. Лаунчер готов работать сразу.
2. Параллельно `tokio::spawn` фоновый `rescan()` — пробежать все sources, обновить SQLite и swap'нуть cache. Типично 300–500 мс на Windows. Пользователь не видит блокировки.
3. Diff stats (`{ added, updated, removed }`) emit'ятся в логи (в будущем — broadcast через command bus → frontend сам обновит список).

Первый запуск без cache — лаунчер показывает только команды расширений ~300 мс, потом список приложений «дополняется» после первого rescan'а.

## Источники Windows

### Start Menu (`.lnk` parsing)

Сканируем рекурсивно две директории:

- `%APPDATA%\Microsoft\Windows\Start Menu\Programs` (per-user)
- `%PROGRAMDATA%\Microsoft\Windows\Start Menu\Programs` (system-wide)

Каждый `.lnk` парсится через [lnk crate](https://crates.io/crates/lnk):

- `link_info().local_base_path()` — target file
- env-var expansion (`%ProgramFiles%`, `%LocalAppData%`, и т.д.)
- skip: uninstaller'ы (по filename regex), `.url`/`.appref-ms`, broken targets, non-executable, zero-byte
- dedup по `sha256(target_lowercase)[..16]`

Display name — **filename без `.lnk`**, не `ShellLink::name()` (часто пуст или дублирует).

### UWP (PackageManager)

`windows::Management::Deployment::PackageManager::FindPackagesByUserSecurityIdWithPackageTypes(.., PackageTypes::Main)` → enum пакетов:

- Skip: `IsFramework`, `IsResourcePackage`, `Status.VerifyIsOK() == Err`
- Skip noise prefixes (`Microsoft.NET.`, `Microsoft.VCLibs.`, `Microsoft.UI.Xaml.`, etc.)
- На каждый Package — `GetAppListEntriesAsync()` → 0..N launchable entries
- `id` = AUMID, `exec_path` = `shell:AppsFolder\<AUMID>`, launch через `cmd /c start "" "<exec_path>"`

## Иконки

Извлекаются eagerly во время `discover()` — у обоих источников есть доступ к источнику ресурса (.lnk file для Win32, `Package` объект для UWP).

### Win32

1. Если `.lnk::icon_location()` задан — используем его. Для Squirrel-installer apps (Discord, Slack, Teams) target = `Update.exe` без embedded icons; реальная иконка прописана в icon_location.
2. Иначе — target `.exe` напрямую.
3. Win32 API: `ExtractIconExW` → `HICON` → `GetIconInfo` → GDI DIB (BGRA bits) → swap → `image` crate PNG encode → файл в icon cache.

### UWP

1. `Package.GetLogo()` или `AppListEntry.DisplayInfo.GetLogo(Size{64,64})` → `RandomAccessStreamReference`
2. `OpenReadAsync()` → bytes → обычно уже PNG
3. **Trim transparent padding** — Microsoft tile assets идут с прозрачными полями для tile rendering, логотип в центре. Без trim'а в 20-пиксельной иконке лаунчера логотип выглядит крошечным. Backend проходит по альфа-каналу, находит bbox непрозрачных пикселей, re-encode'ит PNG.

При extraction failure файл-плейсхолдер **не пишется** — `App.icon_path` остаётся `None`, UI рендерит placeholder div. На следующем rescan'е попытка повторится (раньше placeholder PNG блокировал re-extract, поскольку `ensure_icon` skip'ал «уже извлечённое»).

## Ranking

```
score(query, app, recent_usage)
  = name_match_score(query, app.name) * frecency_boost(app.id, recent_usage)
```

`name_match_score`:

- `1.0` — full prefix (`Note` → `Notepad`)
- `0.7` — word-prefix после space/dash/underscore/dot/slash (`store` → `Microsoft Store`)
- `0.4` — substring
- `0.0` — нет совпадения

`frecency_boost = 1.0 + ln(1 + invocations_30d) * 0.4`. Сейчас в v1 frecency tracking ещё не подключён (TODO: join из ARK `usage_event_obj`), boost всегда `1.0`.

## WS endpoints

| Operation            | Params                                  | Response                                   |
| -------------------- | --------------------------------------- | ------------------------------------------ |
| `app_index.list_all` | `{ limit?: number = 500 }`              | `{ apps: App[] }` с inline base64 иконками |
| `app_index.search`   | `{ query: string, limit?: number = 8 }` | `{ results: ScoredApp[] }` (top-N с score) |
| `app_index.launch`   | `{ id: string }`                        | `{ ok: true }`                             |
| `app_index.rescan`   | (none)                                  | `{ added, updated, removed, total }`       |

Иконки приходят как **inline base64 data URL** в response — renderer не имеет file://-доступа к icon cache. Размер ответа `list_all` ~3–5 MB на типичной машине (65 apps × ~40 KB на иконку) — это в порядке для local IPC, но если станет узким — переедем на `kepler-icon://` custom protocol в main process.

## Что НЕ делаем в v1

- **fs-watch** для Start Menu (notify crate скеleton есть в `watcher.rs`, не wired в supervisor) — нужен рестарт для подхвата новых апок
- **UWP PackageCatalog** events — то же самое
- **Steam / Epic** как отдельные источники (Arrancador уже это делает; дедуп со Start Menu — отдельная задача)
- **`%PATH%` exe scan** (шум: компиляторы, утилиты, библиотечные бинарники)
- **Frecency join** из ARK `usage_event_obj` (присоединится когда usage-tracker начнёт писать события на app launches; пока подключён только к Steam/Epic запускам Arrancador'а)
- **macOS / Linux** реализации (только trait готов)

## Что почитать дальше

- [Write boundary в ARK](./write-boundary) — почему app-индекс **не** в ARK
- [Instance slots](./instances) — slot-aware data directories для app-index.db
- Spec задачи: `.agent/tasks/2026-05-22-app-launcher/spec.md`
