# 2026-05-18 arrancador-full-completion

## Context

`extensions/arrancador/` сейчас read-only: показывает `game_obj` из ARK, но не имеет write paths. Standalone `apps/arrancador/` удалён в Phase A — вся scanner/RAWG/launch/SQOBA логика пропала. Нужно реализовать заново в `kepler-backend` (Rust, потому что filesystem/network/process spawn'инг недоступен extension renderer'у).

Тестовые игры для интеграционных проверок: **Dota 2** (Steam AppID 570), **Cairn** (The Game Bakers, Steam — найти AppID при scan), **Outlast** (Steam AppID 238320).

## Scope

В задаче:

### Backend (`services/kepler-backend/src/arrancador/`)

1. **Scanner** (`scanner.rs`):
   - Steam: парс `libraryfolders.vdf` (стандарт пути `C:\Program Files (x86)\Steam\steamapps\libraryfolders.vdf`), per-library walk `steamapps\appmanifest_*.acf` → name + installdir + size. Detect exe candidates через walk installdir и lookup `*.exe` (приоритет: `<name>.exe`, иначе наибольший .exe).
   - Epic Games Launcher: парс `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON).
   - GOG Galaxy: если установлен, парс `%LOCALAPPDATA%\GOG.com\Galaxy\storage\galaxy-2.0.db` (SQLite). Optional — если DB не найдена, skip.
   - Common: каждый scanner возвращает `Vec<DiscoveredGame { source, name, install_dir, exe_candidate, app_id, store_metadata }>`.
   - Upsert в ARK как `game_obj` через `ark_host.request("upsert_object", ...)`. Match existing по `source + app_id` (props: `source = "steam" | "epic" | "gog"`, `source_app_id`). Не дублировать.

2. **Launcher** (`launcher.rs`):
   - `launch(game_id)` → fetch game_obj → resolve exe_path / steam URL (`steam://rungameid/<id>` для Steam → `start` через ShellExecute) → spawn → return PID + start time.
   - Integration с `usage_tracker` модулем — он уже умеет process detection. По возможности reuse: записать `usage_session` start с `game_id`.

3. **RAWG client** (`rawg.rs`):
   - HTTP GET `https://api.rawg.io/api/games?search=<name>&key=<api_key>` через `reqwest`.
   - `search(query) → Vec<RawgGame { id, name, slug, released, background_image, genres, platforms }>`.
   - `get_details(rawg_id) → RawgGameDetail { ...all + description, screenshots }`.
   - `apply_to_game(game_obj_id, rawg_id)` → fetch details → update game_obj propsJson (`rawg_id`, `cover_image`, `background_image`, `description`, `genres`, `platforms`, `released`).
   - API key хранится в `%APPDATA%\Kosmos\arrancador-config.json` (НЕ в Git, НЕ в логах). Settings UI принимает.

4. **SQOBA** (`sqoba.rs`):
   - `save_paths(game_id) → Vec<PathBuf>` — определяем где лежат сейвы. Heuristics:
     - `game_obj.propsJson.save_paths` если указано явно (manual override).
     - `%USERPROFILE%\Saved Games\<game_name>\`
     - `%USERPROFILE%\Documents\My Games\<game_name>\`
     - `%LOCALAPPDATA%\<game_name>\`
     - Steam Cloud — skip (Steam сам управляет).
   - `backup(game_id) → SqobaBackup { id, game_id, timestamp, dest_path, files_count, bytes }` — zip всех найденных save paths в `%APPDATA%\Kosmos\sqoba\<game_id>\<iso_timestamp>.zip`. Метаданные → `sqoba_backup_obj` объект в ARK (новый тип).
   - `list_backups(game_id) → Vec<SqobaBackup>` — list zip'ов + parse метаданных.
   - `restore(backup_id)` — unzip backup → original save paths. Confirmation handled UI-side.

5. **WS operations** в `ws_server.rs` под namespace `arrancador.*`:
   - `arrancador.scan` (no params) → `{added: N, updated: N, skipped: N, errors: [...]}`
   - `arrancador.launch { game_id }` → `{ok, pid, started_at}` или `{ok: false, error}`
   - `arrancador.rawg.search { query }` → `{results: [...]}`
   - `arrancador.rawg.apply { game_id, rawg_id }` → `{ok}` + emit `entity_changed` event
   - `arrancador.sqoba.backup { game_id }` → SqobaBackup
   - `arrancador.sqoba.list { game_id }` → `{backups: [...]}`
   - `arrancador.sqoba.restore { backup_id }` → `{ok, restored_files, errors}`

6. **Config** (`config.rs`): typed read/write `%APPDATA%\Kosmos\arrancador-config.json` (rawg_api_key, custom scan_paths, sqoba_dest_dir).

### Frontend (`extensions/arrancador/`)

1. **LibraryPage**: кнопка «Запустить» на `GamePosterCard` (или separate action button) → `window.kepler.arrancador.launch(game.id)`. Disable если нет `exe_path`.

2. **ScanPage**: кнопка «Сканировать сейчас» → `arrancador.scan()` → отображение progress (если есть) + результат (N добавлено, N обновлено). История scan'ов (last 10 в localStorage).

3. **CataloguePage**: search-input → `arrancador.rawg.search` → list cards (cover + name + release). Кнопка «Применить metadata» (если игра уже в библиотеке — apply через `arrancador.rawg.apply`). Если игра не в библиотеке — пока в roadmap, но stub'нуть «add to library» через manual create game_obj.

4. **SqobaPage**: per-game list backups → кнопки «Создать бекап» / «Восстановить» (с confirmation). Empty state если игр нет или sqoba.list пуст.

5. **SettingsPage**: новый input «RAWG API ключ» (password field) → store через `arrancador.config.set_rawg_key`. Visible status «ключ задан / не задан».

6. **Preload** (`shell/electron/extension-host.ts` — `kepler.arrancador.*` namespace в extension preload):
   ```ts
   window.kepler.arrancador = {
     scan: () => Promise<ScanResult>,
     launch: (game_id: string) => Promise<LaunchResult>,
     rawg: { search, apply },
     sqoba: { backup, list, restore },
     config: { get_rawg_key, set_rawg_key },
   };
   ```

### Tests

1. **Rust unit tests** в каждом backend модуле:
   - `scanner_steam_parses_libraryfolders_vdf` — fixture VDF из `tests/fixtures/steam/libraryfolders.vdf` (synthetic).
   - `scanner_steam_parses_appmanifest_acf` — fixture с тремя играми (Dota 2 / Cairn / Outlast).
   - `scanner_epic_parses_manifest_item` — fixture JSON.
   - `launcher_resolves_steam_url` — `steam://rungameid/570` для Steam-source game.
   - `rawg_client_search_serializes_query` — mock HTTP response.
   - `sqoba_backup_zips_files` — temp dir + sample files → zip → unzip → verify content matches.
   - `sqoba_restore_overwrites_originals` — backup → modify original → restore → verify reverted.

2. **Integration test** (Rust): `arrancador_full_flow`:
   - Fake Steam library структура в temp dir (3 .acf файла: Dota 2 / Cairn / Outlast).
   - Env override `STEAM_LIBRARY_FOLDERS=<temp_dir>` (override constant в config).
   - `scan` → asserts 3 game_obj в ARK с правильными именами + source_app_id.
   - RAWG mock через WireMock-rs (или httpmock): `apply` → game_obj propsJson обновлён.
   - `sqoba.backup` → файл создан в temp.

3. **Manual smoke** (на твоей машине): обнаружить реальные Dota 2 (570), Cairn, Outlast (238320). Запустить scan, проверить что они появились в LibraryPage. Применить RAWG к одной игре. Сделать SQOBA backup. Restore.

### Документация

- `docs-site/apps/arrancador.md` — обновить полностью (text сейчас обзорный standalone), добавить разделы scanner / launcher / RAWG / SQOBA, что в backend, что в extension renderer.
- `docs-site/concepts/` — новая страница `arrancador-backend.md` (или встроить в существующую страницу) — описание namespaced WS operations.
- `STATUS.md` — Arrancador в Сделано.
- `docs-site/apps/kepler-roadmap.md` — Arrancador completion ⏳ → ✅.
- `bun run docs:sync && bun run docs:check` зелёные.

Не в задаче:

- **Mac/Linux scanner pathways** — пока только Windows.
- **Tracking игр на 3rd-party launcher'ах** (Battle.net, Riot, EA) — отдельная Phase 4.X.
- **Cloud sync саве-файлов** — Phase 11 backup перекрывает на уровне ARK.
- **In-game overlay** — нет.
- **Achievement tracking** — нет.
- **Auto-update Cairn appid resolution через RAWG** при scan — Cairn будет добавлен как Steam game с правильным `source_app_id`; RAWG matching apply'ится через CataloguePage manually.

## Acceptance Criteria

**AC1.** `services/kepler-backend/src/arrancador/{mod,scanner,launcher,rawg,sqoba,config}.rs` существуют, экспортированы из `lib.rs`.

**AC2.** Каждый module имеет ≥2 unit tests. Все `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib` зелёные.

**AC3.** Integration test `arrancador_full_flow` зелёный — синтетическая Steam library с Dota 2/Cairn/Outlast scanned, 3 game_obj в ARK с корректными именами + RAWG mock apply работает + SQOBA backup создан.

**AC4.** WS operations `arrancador.*` доступны через `ark_host.request(...)`. UI subagent смог их вызвать в `window.kepler.arrancador.*`.

**AC5.** `extensions/arrancador/src/pages/*.vue`: LibraryPage button «Запустить», ScanPage кнопка scan, CataloguePage RAWG search/apply, SqobaPage backup/restore, SettingsPage RAWG key field. Все используют `@kepler/visuals` где уместно (Toggle, SettingsRow, EmptyState).

**AC6.** `bun run --cwd shell typecheck` clean.

**AC7.** `bun run --cwd shell build:extensions` clean.

**AC8.** `bun run ark:guard:writes` clean (write paths идут через `upsert_object` / `delete_object` через WS, не raw SQL).

**AC9.** `bun run test:e2e` зелёный (existing 20/20). Никаких новых e2e в этот loop (manual smoke достаточен для интеграции).

**AC10.** Documentation: `docs-site/apps/arrancador.md` updated, `STATUS.md` + `kepler-roadmap.md` updated, `bun run docs:sync && bun run docs:check` зелёные.

**AC11.** Manual smoke (operator) с реальными Dota 2 / Cairn / Outlast: scan находит все три, launch одной из них работает, RAWG apply к одной из них работает, SQOBA backup для одной → restore.

## Verification commands

См. AC. Specific:
- `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib`
- `cargo test --manifest-path services\kepler-backend\Cargo.toml --test arrancador_full_flow` (если integration через `tests/` directory)
- `bun run --cwd shell typecheck`
- `bun run --cwd shell build:extensions`
- `bun run ark:guard:writes`
- `bun run test:e2e`
- `bun run docs:sync && bun run docs:check`

## Out of scope decisions

- **Steam path discovery** — на Win10/11 стандартный `C:\Program Files (x86)\Steam\` + reading reg key `HKCU\Software\Valve\Steam\SteamPath` как fallback. Hardcode для MVP, registry — TODO follow-up.
- **RAWG rate limiting** — RAWG free tier: 20k req/month. Не реализуем local rate-limit в первой версии; если упрёмся — 429 handle с retry.
- **Launch tracking** — kepler-backend `usage_tracker` модуль уже делает process polling. Reuse — launcher просто spawn'ит + назначает `usage_session.game_id`, tracker сам отметит exit.
- **SQOBA versioning** — keep N последних backup'ов (configurable, default 10), удалять старые при `backup()`.
- **API key encryption** — `arrancador-config.json` хранит в plaintext. Не secret-grade — это user API key, не credentials. Если потребуется encrypt — `keyring-rs` отдельной фазой.
- **Cairn AppID** — определится при scan'е через RAWG match (или manual). Не hardcode'им в test fixtures как constant — fixture использует synthetic 1810770 (примерное значение).
