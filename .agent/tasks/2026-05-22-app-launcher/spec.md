# 2026-05-22 app-launcher

## Context

Поисковик Kepler (Alt+Space) сейчас находит только команды расширений (Eden / Delphi / Horologion / Arrancador) через command bus. **Запуска установленных приложений Windows нет** — пользователь не может из лаунчера открыть Chrome, Notepad++, Visual Studio Code, Discord, Steam UWP-приложение и т.д.

Это core feature любого launcher'а (Raycast, PowerToys Run, Wox/Flow Launcher, Spotlight). Без неё Kepler — недо-лаунчер.

Дополнительная цель — **cross-platform-ready архитектура**. Сейчас делаем Windows, но trait + per-platform impl должны позволить добавить macOS / Linux без переписывания общей логики.

Research (`a6d57d15...`): все production launcher'ы используют **отдельный cached storage + fs-watch invalidation**. Windows Search Indexer все избегают (ненадёжен, медленный cold, плохо покрывает UWP). Best practice — own SQLite + fs-watch + memory cache.

## Scope

### Backend — `services/kepler-backend/src/app_index/`

1. **Common module (`mod.rs`, `app.rs`, `store.rs`, `cache.rs`, `icons.rs`, `watcher.rs`)** — platform-agnostic.

   - `pub trait AppSource: Send + Sync { fn name(&self) -> &str; fn discover(&self) -> Result<Vec<App>>; fn launch(&self, app: &App) -> Result<()>; }`
   - `struct App { id: String, name: String, exec_path: String, icon_path: Option<String>, kind: AppKind, source: String, mtime: i64 }`
   - `enum AppKind { Win32, Uwp, MacBundle, LinuxDesktop }`
   - `store.rs` — SQLite `app-index.db` в `<instance.dataDir>/app-index.db` (рядом с `ark.db`, изоляция per-slot через `resolveInstance`). Schema: `apps(id PK, name, exec_path, icon_path, kind, source, mtime, last_indexed_at)` + индекс по `name COLLATE NOCASE` для prefix search.
   - `cache.rs` — `Arc<RwLock<Vec<App>>>` in-memory. Hot path для search — <10 ms.
   - `icons.rs` — extract `.ico` из `.exe`/`.lnk` через Win32, конвертация в PNG, кэш в `<instance.localDataDir>/app-icons/<sha256(exec_path)>.png`. На запуске только проверяем `path.exists()`; если иконка пропала — emit `app_index.icon_missing` для refresh.
   - `watcher.rs` — `notify` crate на Start Menu dirs + TTL fallback 6h. UWP watcher — отдельно в `platform/windows/uwp.rs` через `PackageCatalog`.

2. **Windows impl — `platform/windows/start_menu.rs`**:
   - Рекурсивный scan двух директорий:
     - `%APPDATA%\Microsoft\Windows\Start Menu\Programs`
     - `%PROGRAMDATA%\Microsoft\Windows\Start Menu\Programs`
   - Каждый `.lnk` parse через crate `lnk` (если ok) или `IShellLinkW` через `windows` crate (fallback на броken/relative .lnk). Достать `name` (из filename без .lnk), `target_path`, expand env vars (`%ProgramFiles%`, `%LocalAppData%`).
   - Skip uninstaller'ы (фильтр по name regex `(?i)(uninstall|deinstall|удалить)`), shortcut'ы на URL (`.url` skip пока), broken target (path не существует).
   - Launch: `std::process::Command::new("cmd").args(["/c", "start", "", &target_path])` (works для .exe + .bat + browser .url; для UWP отдельно).

3. **Windows impl — `platform/windows/uwp.rs`**:
   - `windows` crate, `windows::Management::Deployment::PackageManager`.
   - `PackageManager::FindPackagesForUser(None)` → enum installed packages → для каждого AppListEntry → `DisplayInfo.DisplayName`, `DisplayInfo.GetLogo()`, AUMID.
   - Skip non-launchable (background extensions), system packages (filter по `IsFramework`, `SignatureKind`).
   - Launch: `cmd /c start "" "shell:AppsFolder\<AUMID>"`.
   - UWP icon — extract из `Package.InstalledLocation` + manifest, копия в app-icons cache.

4. **WS endpoints в `ws_server.rs`** под namespace `app_index.*`:
   - `app_index.search { query: String, limit?: usize }` → `{ results: [{ id, name, icon_path, kind, score }] }`
   - `app_index.launch { id }` → `{ ok: true }` + emit `usage_event_obj` в ARK (frecency tracking)
   - `app_index.rescan` (force re-index) → `{ ok, added, updated, removed }`
   - Event broadcast `app_index.updated` после rescan'а — frontend перерисовывает закэшированный список.

5. **Frecency join** — отдельный модуль `app_index/ranking.rs`:
   - `score(query, app, recent_usage) = name_match_score(query, app.name) * frecency_boost(app.id, recent_usage)`
   - `name_match_score` — простой prefix + substring scoring (без BM25 пока, добавим если нужно)
   - `frecency_boost(id, usage_events) = 1.0 + log(1 + recent_invocations) * recency_factor`
   - `usage_event_obj.subject_kind = "app"`, `subject_id = App.id` (новый use-case, existing schema это допускает)

### Frontend — `shell/src/views/LauncherView.vue`

1. **Query → search** — debounce 50ms, вызвать `arkClient.request("app_index.search", { query, limit: 8 })`.
2. **Render results** — после existing command-bus результатов, секция «Приложения» с иконкой + name. Use `img` с `src="file:///<icon_path>"` через protocol handler (или зарегистрировать custom `kepler-icon://` protocol в main process).
3. **Enter / click** — `arkClient.request("app_index.launch", { id })`. Лаунчер скрывается.
4. **Empty-state на первый запуск** (cache холодный) — text «Индексирую приложения…» + spinner. Listener на `app_index.updated` для refresh.

### Tests

1. **Unit tests** (`services/kepler-backend/src/app_index/`):
   - `.lnk` parsing — fixture `.lnk` файл (commit'нем небольшой в `services/kepler-backend/tests/fixtures/`), assert `target_path`, `name`.
   - UWP enum — mocked PackageManager (or skip — Win-only integration test, рассмотреть).
   - Icons — extract test .ico, assert PNG output size > 0.
   - SQLite store — round-trip apps[], assert ordering, COLLATE NOCASE.
2. **Integration smoke** — `tests/e2e/launcher-app-search.spec.ts`:
   - Открыть launcher → ввести «notepad» → assert results contains «Блокнот» (Windows встроенный, всегда есть).
   - Click → assert `usage_event_obj` создан в ARK.

## Out of scope

- macOS / Linux **impl** (только trait готовим). Файлы `platform/{macos,linux}.rs` создаются с `unimplemented!()` или удаляются — лучше пока не создавать вообще, добавим при первой работе под macOS.
- Steam / Epic как отдельный AppSource — Arrancador уже это делает. Дедуп со Start Menu / UWP обсудим в v2.
- `%PATH%` exe scan (шум).
- Browser bookmarks / files search / clipboard history (будущие core-features).
- Fuzzy search (для v1 — prefix + substring достаточно).
- Themed/light icons (только raw extract).

## Acceptance criteria

1. ✅ Backend компилится `cargo build --manifest-path services/kepler-backend/Cargo.toml --bin kepler-backend`.
2. ✅ Все existing unit tests проходят (`cargo test --lib`) + новые добавленные тесты для app_index.
3. ✅ После запуска Kepler dev (`bun run --cwd shell dev`):
   - В лог backend'а видно сообщение типа `[app_index] indexed N apps in Mms` (N ≥ 50 на стандартной Windows-машине).
   - В `<instance.dataDir>/app-index.db` появляется файл.
   - В `<instance.localDataDir>/app-icons/` появляются PNG файлы.
4. ✅ Открыть launcher (Alt+Space dev hotkey) → ввести «notepad» → видны результаты с иконкой → Enter → Блокнот запускается.
5. ✅ Открыть launcher → ввести «calc» → видны Calculator (UWP) с иконкой.
6. ✅ `bun run ark:guard:writes` зелёный (мы не пишем в ARK-таблицы напрямую — только через `record_local_upsert` для usage_event_obj).
7. ✅ `bun run ark:smoke` зелёный.
8. ✅ Build full Kepler installer (`bun run --cwd shell build`) проходит (новые crate deps корректно встают в Cargo.lock).

## Architecture decisions

### Почему отдельный SQLite (`app-index.db`), не ARK?

- App index — **host-specific и regenerable**. Пути отличаются между машинами (User profile, install location), нет смысла синхронизировать.
- Не загрязняем `sync_version_vector` событиями fs-watch (Start Menu меняется при каждой установке/удалении приложения).
- Соответствует духу `docs-site/concepts/write-boundary.md`: ARK = user data, app-index = derived host state.
- Frecency tracking (`usage_event_obj`) **остаётся в ARK** — это user behaviour data, имеет смысл синхронизировать. Join в-памяти при `app_index.search`.

### Почему trait AppSource, не один монолит?

- Cross-platform абстракция: Windows = `[StartMenuSource, UwpSource]`, macOS позже = `[SpotlightSource]`, Linux = `[XdgDesktopSource]`.
- Внутри платформы тоже легко добавить (Steam через Arrancador, browser bookmarks, etc.) — каждый источник самодостаточный.
- Trait метод `name()` — для логирования и метрик per-source.

### Почему SHA256(exec_path) для icon filename?

- Stable id для иконки, инвариант к переименованиям .lnk.
- Collision-free для практических сценариев.
- Easy invalidation: если файл .exe изменился (mtime) — пересчитываем хэш target'а, видим что иконка соответствует другому состоянию.

### Что НЕ делаем в этой задаче

- Sync app preferences между устройствами (favorites, hidden apps) — это в ARK позже как `app_preferences_obj`.
- Indexing other sources (browser bookmarks, recent files, clipboard) — каждое отдельный proof loop.

## Proof loop

При завершении сравнить с AC, заполнить `evidence.md` со снимками: build log, sample of indexed apps, screenshot launcher с notepad search.

## Estimate (от skill estimate-calibration)

- **Human gut:** ~60h (~7-10 рабочих дней с ревью + integration)
- **LLM wall-clock:** ~25 мин (anchored на arrancador-full-completion и focus-mode-full-impl)
