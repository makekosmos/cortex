# Evidence — 2026-05-24 File Search scopes, ignores, NTFS mode

Verified at: 2026-05-24T13:38:00+03:00.

## AC1 — persisted settings payload

**PASS.** Implemented in `FileStore` + `FileIndex::settings`: roots, ignore patterns, `respect_gitignore`, `include_hidden`, `ntfs_accelerated`, and backward-compatible `exclude_noisy_folders`.

Evidence:

- `services/kepler-backend/src/file_index/store.rs::tests::roots_and_ignore_patterns_are_persisted_and_deduped`
- `services/kepler-backend/src/file_index/store.rs::tests::new_file_search_settings_have_safe_defaults`
- `cargo test -p kepler-backend file_index::` → PASS, 16/16.

## AC2 — Search Scopes UI add/remove

**PASS.** `SettingsView.vue` now renders scope list, uses `window.kepler.fileSearch.pickScope()`, and calls `scopeAdd/scopeRemove`. Backend validates directories, persists roots, restarts watcher, rescans, and schedules removed-subtree cleanup in the background so deleting a large scope does not block IPC.

Evidence:

- `services/kepler-backend/src/file_index/mod.rs::tests::removing_root_hides_scope_immediately_and_cleans_index_in_background`
- `services/kepler-backend/src/file_index/mod.rs::tests::scope_remove_does_not_cleanup_large_index_synchronously`
- `bun run --cwd shell typecheck` → PASS.

## AC3 — Ignore patterns UI add/remove

**PASS.** Settings UI has add/remove list for ignore patterns. Backend validates blank/invalid globs, dedupes via primary key, persists patterns, and rescans.

Evidence:

- `services/kepler-backend/src/file_index/mod.rs::tests::ignore_patterns_filter_matching_files`
- `services/kepler-backend/src/file_index/store.rs::tests::roots_and_ignore_patterns_are_persisted_and_deduped`
- `bun run --cwd shell typecheck` → PASS.

## AC4 — hardcoded noisy paths ignored

**PASS.** User-mode scanner applies root-relative hardcoded ignores for `node_modules`, `.git`, `target`, `dist`, `build`, `AppData`, cache/temp, etc.

Evidence:

- `services/kepler-backend/src/file_index/mod.rs::tests::scan_searches_regular_files_and_skips_noisy_folders_by_default`
- `cargo test -p kepler-backend file_index::` → PASS.

## AC5 — `.gitignore` toggle

**PASS.** `ScanOptions.respect_gitignore` is persisted and wired into `ignore::WalkBuilder` via `git_ignore`, `git_global`, `git_exclude`, and `parents`.

Evidence:

- `cargo test -p kepler-backend file_index::` → PASS.
- `bun run --cwd shell typecheck` → PASS.

## AC6 — hidden files toggle

**PASS.** `include_hidden` defaults OFF and is wired through scanner and watcher. Dot-hidden paths are excluded by default; Settings exposes the toggle.

Evidence:

- `services/kepler-backend/src/file_index/scanner.rs::default_roots_tests::hidden_dot_paths_are_excluded_by_default`
- `cargo test -p kepler-backend file_index::` → PASS.

## AC7 — NTFS accelerated mode toggle

**PASS.** `ntfs_accelerated` defaults OFF. Scanner only attempts existing NTFS fast path when the toggle is ON and root is a drive root; otherwise user-mode scan is used. Existing fallback to walk mode remains.

Evidence:

- `services/kepler-backend/src/file_index/scanner.rs`
- `cargo test -p kepler-backend file_index::` → PASS.

## AC8 — watcher/root consistency

**PASS.** `FileIndex` now stores watcher behind a mutex and restarts it after scope add/remove. Watcher reads current options from store for events.

Evidence:

- `services/kepler-backend/src/file_index/mod.rs::restart_watcher`
- `services/kepler-backend/src/file_index/watcher.rs::scan_options`
- `services/kepler-backend/src/file_index/mod.rs::tests::removing_root_hides_scope_immediately_and_cleans_index_in_background`

## AC9 — targeted Rust tests

**PASS.**

Command:

```powershell
cargo test -p kepler-backend file_index::
```

Result: 18 passed, 0 failed.

## AC10 — shell typecheck

**PASS.**

Command:

```powershell
bun run --cwd shell typecheck
```

Result: `tsc --noEmit` passed.

## AC11 — ARK write guard

**PASS.**

Command:

```powershell
bun run ark:guard:writes
```

Result: `ARK write boundary guard passed.`

## AC12 — postmortem

**PASS.** `docs-site/agents/postmortems.md` updated from unresolved to fixed with fix, regression protection, and prevention.

## Additional substantial-task verification

**PASS.**

Command:

```powershell
bun run ark:smoke
```

Result: `ARK smoke matrix passed.`

**PASS.**

Command:

```powershell
bun run docs:check
bun run docs:sync
bun run docs:check
```

Result: docs fresh, AGENTS/CLAUDE sync completed.

## Follow-up fix — settings timeout during initial rescan

**PASS.** Manual dev run showed `kepler:file-search:settings:get` timing out while initial `file_index` rescan was still bulk-inserting. Root cause: settings reads shared the same `Mutex<Connection>` as `replace_all`. `FileStore` now keeps `path` and opens a separate read-only SQLite connection for settings/roots/pattern reads, so WAL readers are not blocked by the long writer mutex.

Command:

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index::
```

Result: 16 passed, 0 failed.

## Follow-up fix — scope remove timeout during long rescan

**PASS.** Manual dev run showed `file_index.scope_remove` timing out while a long `D:\` NTFS/full rescan was active. Root cause: settings mutations waited on `scan_lock`. `FileIndex` now uses a scan generation counter: settings mutations invalidate the running scan and return without waiting for `scan_lock`; the stale scan discards its results before `replace_all`.

Regression:

- `services/kepler-backend/src/file_index/mod.rs::tests::scope_remove_does_not_wait_for_running_rescan_lock`

Command:

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index::
```

Result: 17 passed, 0 failed.

## Follow-up fix — file_index.rescan blocking WS dispatch

**PASS.** Manual dev run showed `file_index.settings_get` timing out even after initial scan logs, because `file_index.rescan` was still a synchronous WS operation: the single ArkClient WS dispatch loop could not read later `settings_get` frames until the long rescan request returned. `file_index.rescan` now schedules background scanning and returns current stats immediately. `FileIndexSettings` includes `scan_in_progress`; Settings polls that field and shows toast/progress UI through `@kosmos/visuals`.

Commands:

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index::
bun run --cwd shell typecheck
```

Result: backend file_index 17 passed, shell typecheck passed.

## Follow-up audit pass — 2026-05-24 evening

Глубокий аудит (3 sub-agents) выявил 12 серьёзных багов в Phase 1. Зафикшены и покрыты regression-тестами:

### C1 — watcher применял события к удалённым scopes

**Root cause.** `notify::RecommendedWatcher` ставит события в очередь; между `invalidate_running_scan` и `restart_watcher` в канале остаются события для уже удалённого root. `apply_path` фильтровал только по `should_index_with_options`, не по принадлежности к активным roots → `store.upsert` гонит файлы обратно после background cleanup.

**Fix.** `watcher::handle_event` теперь читает `store.roots()` на каждый event и `path_is_under_any_root` отбрасывает события вне активных scopes. Сравнение case-insensitive + защита от false-prefix-match (`D:\Active` vs `D:\ActiveBackup`).

**Regression test.** `file_index::watcher::tests::events_outside_active_roots_are_dropped`.

**Prevention.** Watcher state и settings state — две независимые подсистемы; на стыке всегда нужна явная фильтрация по текущему snapshot настроек, не предполагать что restart_watcher успеет до доставки очередного события.

### C2 — toggle «скрытые файлы» не уважал NTFS-attribute

**Root cause.** `is_dot_hidden` смотрит только на дот-префикс пути; на Windows hidden — это бит `FILE_ATTRIBUTE_HIDDEN` (0x2) в метаданных. `AppData`, `ntuser.dat`, `Thumbs.db` без точки в имени просачивались через NTFS fast-path и watcher events.

**Fix.** Добавлен `path_has_hidden_attribute` через `std::os::windows::fs::MetadataExt::file_attributes()`. Проверяется HIDDEN | SYSTEM (system files в Explorer тоже скрыты).

**Regression test.** `file_index::scanner::default_roots_tests::ntfs_hidden_attribute_excludes_file_when_include_hidden_off` (создаёт hidden файл через `attrib +H`, проверяет включение/выключение).

**Prevention.** «Hidden» — платформо-зависимое понятие; нельзя ограничиваться dot-convention из POSIX-мира.

### H2 — NTFS fast-path не активировался для picker-вариантов пути

**Root cause.** `is_drive_root` сравнивал byte-length с 3 (для `"D:\"`). Picker может вернуть `D:`, `D:\\`, `D:\\\\`, `d:/` — все они мимо проверки.

**Fix.** `trim_end_matches(['\\', '/'])` + проверка ровно 2 байт: буква + `:`.

**Regression test.** `file_index::scanner::default_roots_tests::drive_root_detection_accepts_picker_variants`.

**Prevention.** Path equality на Windows никогда не делается через byte-comparison; всегда trim trailing separators и нормализовать casing.

### H4 — SQLITE_BUSY на settings_get во время rescan

**Root cause.** `read_conn()` открывал новое SQLite-соединение без `busy_timeout`. WAL не спасает: writer держит транзакцию на длинных `replace_all`, reader SQLITE_BUSY-ит сразу.

**Fix.** `conn.busy_timeout(5s)` в `read_conn()`.

**Prevention.** Любое SQLite-соединение в multi-connection setup должно явно конфигурить `busy_timeout`. PRAGMA defaults — 0ms (немедленный fail).

### H5 — удаление scope без подтверждения

**Root cause.** `onRemoveFileSearchScope` сразу вызывал backend; необратимая операция (выполняется фоновая чистка тысяч файлов из индекса) без warning.

**Fix.** `window.confirm` с явным текстом «все её проиндексированные файлы будут удалены».

**Prevention.** Любая операция с irreversible side-effect и стоимостью > секунды — confirm. Чек-лист дисциплины перед merge UI features.

### H6 — английский в user-facing UI

**Root cause.** Скопированный из плана английский «Ignore patterns» в заголовке + слово «Pattern» в toast-messages.

**Fix.** «Шаблоны исключений», «Шаблон добавлен/удалён/уже добавлен».

**Prevention.** Жёсткое правило CLAUDE.md: user-facing strings — только русский. Скопировал из спеки — переведи.

### H7 — backend errors глоталась в generic toast

**Root cause.** Catch-блоки писали `console.warn(err)` и присваивали generic `fileSearchError`. Backend reason (например «pattern уже есть») терялся.

**Fix.** `describeFileSearchError` распознаёт известные маркеры (`pattern уже есть`, `invalid ignore pattern`, etc.) и показывает backend message; иначе суффикс к generic-сообщению. Дополнительно frontend-side dedup до отправки запроса (мгновенный feedback).

**Regression test.** Бэкенд: `file_index::store::tests::ignore_pattern_dedup_is_case_insensitive` — `add_ignore_pattern("*.TMP")` после `*.tmp` возвращает `InvalidSetting`, не silent no-op.

**Prevention.** Error catch — это не «show generic», а «понять или показать сырое». Никогда не `console.warn(err)` без проброса в UI.

### M3 — дубликаты ignore patterns по регистру

**Root cause.** SQLite PRIMARY KEY case-sensitive. `*.tmp` и `*.TMP` — две строки.

**Fix.** Explicit `lower()` lookup перед insert; при дубликате — `InvalidSetting` ошибка. `remove_ignore_pattern` тоже case-insensitive.

**Regression test.** `file_index::store::tests::ignore_pattern_dedup_is_case_insensitive`.

### M4 — дубликаты roots по регистру и trailing slash

**Root cause.** Те же причины + picker возвращает разные варианты. `D:\Personal` vs `D:\Personal\` vs `d:\personal` = три watcher-инстанса на одно дерево → дубли upsert.

**Fix.** `normalize_root` (trim trailing slashes, кроме drive root `D:\` где `\` обязателен для metadata API) + case-insensitive INSERT/DELETE через `lower()`.

**Regression tests.** `file_index::store::tests::roots_dedup_is_case_and_trailing_slash_insensitive`, `drive_root_normalization_keeps_trailing_backslash`.

**Prevention.** Path как ключ — всегда canonicalize в одной точке (на write). Никаких «store as-given, compare with lower()» — пишем уже нормализованным.

### M5 — `useToast.update({ duration })` taймер leak

**Root cause.** `update` запускал новый `setTimeout(dismiss)`, не отменяя старый. Toast скрывался по первому таймауту → пользователь видел исчезновение раньше времени.

**Fix.** `Map<id, timerId>` + `scheduleDismiss` отменяет предыдущий перед новым. `dismiss` чистит из map.

**Prevention.** При scheduling-операциях с overridable timeout всегда тримить предыдущий handle. Тест для toast composable — pending follow-up.

### M7 — кнопка «Переиндексировать» не учитывала server-side `scan_in_progress`

**Root cause.** Локальный `fileSearchBusy` не отражает background watcher/rescan. Кликом юзер мог поставить N rescan-jobs в очередь на `scan_lock`.

**Fix.** Disabled-условие включает `fileSearchSettings?.scan_in_progress`; ранний `return` с UI-сообщением «Индексация уже идёт».

**Prevention.** Server-side state — source of truth для дисейбла, не local UI flag.

### M9 — native dialog blocked headless e2e

**Root cause.** `pickScope` IPC handler открывал `dialog.showOpenDialog` без headless-guard. Под `KOSMOS_HEADLESS=1` тест бы завис.

**Fix.** Early `return null` если `process.env.KOSMOS_HEADLESS === "1"`.

**Prevention.** Любой нативный dialog/window в `shell/electron/*` — обязательная headless-проверка. Правило в CLAUDE.md уже есть, был забыт при добавлении новой команды.

### M10 — `validate_ignore_pattern` пропускал invalid globs

**Root cause.** `Glob::new` validate-only синтаксис parsing-уровня; `[abc` парсится, но падает на `GlobSetBuilder.build`. Backend `add_ignore_pattern` принимал и persist'ил, scanner потом silently дропал паттерн.

**Fix.** Полный pipeline: `Glob::new` → `GlobSetBuilder::add` → `.build()`. Все три уровня должны успеть.

**Regression test.** `file_index::scanner::default_roots_tests::validate_ignore_pattern_rejects_unbuildable_globs`.

**Prevention.** Validation должна повторять production path в миниатюре, не только первый этап.

### Команды финальной проверки

```powershell
cargo test -p kepler-backend --lib file_index::
bun run --cwd shell typecheck
bun run ark:guard:writes
```

Результат: file_index 25/25 passed, shell tsc clean, ARK write boundary guard passed.

### Второй проход — оставшиеся CRITICAL+HIGH+MEDIUM закрыты

После первого батча закрыты ещё 10 проблем:

### C3 — rescan committed stale snapshot under generation race

**Root cause.** Окно между check (`scan_generation.load == generation`) и `replace_all` — мутация (`set_settings`/`remove_root`) могла бампнуть generation и записать stale данные. Eventual consistency формально была через `spawn_rescan` от мутации, но в worst-case два task'а конкурируют за scan_lock в обратном порядке (мутация захватила, новый rescan ждёт).

**Fix.** После `replace_all` ещё одна проверка generation; mismatch → автоматический `spawn_rescan`. Coalescing (H3) защищает от лавины.

**Regression test.** `file_index::tests::rescan_schedules_followup_when_generation_changes_during_write`.

**Prevention.** Если check невозможно сделать atomic с write, нужен self-healing follow-up. Никаких «надеемся что не случится».

### H1 — NTFS fast path игнорировал `.gitignore`

**Root cause.** NTFS scan читает MFT, не обходит дерево — нет места куда вставить `.gitignore` matching.

**Fix.** Когда `respect_gitignore=true` И `ntfs_accelerated=true`, NTFS path skip'ается, fallback на user-mode walk (где gitignore работает). Backend сообщает `NtfsStatus::Fallback` с note «Учитывается .gitignore — NTFS режим не активен». UI hint описывает это явно.

**Prevention.** Когда два feature-флага дают конфликтующую семантику, явный конфликт-резолвер (один проиграл, который — задокументировано) лучше тихого silent-bypass одного из них.

### H3 — toggle storm спавнил N rescan-jobs

**Root cause.** Каждая мутация `set_settings`/`add_root`/`remove_root` вызывает `spawn_rescan`. 5 переключений → 5 параллельных `tokio::spawn`'ов в очереди на `scan_lock`. Они выполнятся последовательно, тратя время и I/O.

**Fix.** `AtomicBool rescan_pending` в `FileIndex`. `spawn_rescan` atomically swap'ает true; если был true — return. Внутри task'а сразу после `tokio::spawn` сбрасывает в false, чтобы следующая мутация во время уже-ждущей rescan могла поставить новую в очередь.

**Regression test.** `file_index::tests::rescan_coalesces_overlapping_spawn_requests` (50 spawn-вызовов подряд → один rescan).

**Prevention.** Любой debounce/coalesce — лучше через atomic swap, не через `Mutex<bool>` (proне поломок легче, lock-free).

### H8 — `pickScope` без guard на root-drive / UNC

**Root cause.** Никаких ограничений: юзер мог легко добавить `C:\` (загрузочный диск) или `\\server\share` (сетевая FS, документировано как out-of-scope).

**Fix.** IPC handler `scope:add` отклоняет UNC (`startsWith("\\\\")` / `//`). Для drive root (`D:`, `C:\\`) показывает confirm-dialog с предупреждением о масштабе сканирования и стандартных исключениях. Headless-режим bypass'ит confirm (тесты).

**Prevention.** Любая операция, потенциально затрагивающая «весь диск» или «сетевой ресурс» — обязательное предупреждение пользователю до выполнения.

### H9 — `settings:set` IPC handler принимал произвольные ключи

**Root cause.** `...patch` спред'ил `Record<string, unknown>` в payload. Backend получал любой ключ из renderer'а (renderer compromised → arbitrary settings injection).

**Fix.** Explicit allowlist boolean-полей: `exclude_noisy_folders`, `respect_gitignore`, `include_hidden`, `ntfs_accelerated`. Всё остальное игнорируется. Non-boolean значения откидываются.

**Prevention.** IPC boundary — это trust boundary; никогда не спред'ить произвольные объекты от renderer'а в backend payload.

### H10 — NTFS toggle без feedback

**Root cause.** Юзер включает «Ускоренный NTFS», и не понимает: реально активен? fallback? недоступен?

**Fix.** Новое поле `NtfsStatus` в `FileIndexSettings` (`unknown | disabled | active | fallback | unavailable`). Scanner репортит после каждого scan. UI показывает статус под toggle.

**Prevention.** Любой переключатель с silent-fallback должен иметь status indicator. Иначе юзер не отличает работу от тишины.

### M1 — `exclude_noisy_folders=false` отключал все defaults включая AppData

**Root cause.** `ignore_matcher` гейтил DEFAULT_IGNORE_PATTERNS через `exclude_noisy_folders`. Логика «выключаю шум → включаю все хардкод-исключения» спутывала **имена папок** (`.git`, `node_modules` — должны слетать) с **path-патернами** (`**/AppData/**` — должны оставаться всегда).

**Fix.** Разделение концепций: NOISY*FOLDER_NAMES (компонент-имена) контролируется toggle, DEFAULT_IGNORE_PATTERNS (только AppData/Cache/*.tmp/\_.temp) — всегда. Из defaults убраны дубли (`.git`, `target`, `dist`, `build`, `.next`, `.nuxt`, `__pycache__`, `tmp`, `temp` — уже в NOISY_FOLDER_NAMES).

**Prevention.** Не складывать два разных feature под один toggle. Если значения связаны — выделить «meta-toggle» с явной composition logic.

### M2 — pessimistic toggle visual flip

**Root cause.** Checkbox `:checked` напрямую биндился на `fileSearchSettings?.field`. После клика state не менялся локально → визуально отскок к серверному значению до `loadFileSearchSettings()`.

**Fix.** Optimistic update — мутируем `fileSearchSettings.value` сразу после клика. В catch'е восстанавливаем snapshot из перед-await состояния.

**Prevention.** Pessimistic updates с round-trip — UX-плохой паттерн для бинарных toggle'ов. Optimistic + explicit rollback на error — стандарт.

### M6 — `clearFileSearchPoll` не dismiss'ил toast

**Root cause.** Очищал timer но toast оставался жить с `duration:0` навсегда. Новая операция → новый toast создаётся поверх → stack растёт.

**Fix.** `clearFileSearchPoll` теперь `toast.dismiss(fileSearchToastId)` + сброс id.

**Prevention.** Cleanup-функция должна быть симметричной к setup'у — если setup создаёт A+B, teardown убирает оба.

### M8 — misleading «Папки не выбраны» во время начальной загрузки

**Root cause.** При `fileSearchSettings === null` (WS ещё не ответил) UI рендерил пустые списки и сообщение «Папки не выбраны — поиск ничего не индексирует» — что было ложью.

**Fix.** Отдельная ветка `v-if="fileSearchSettings === null && !fileSearchError"` показывает «Загрузка настроек поиска…». Все остальные секции под `v-if="fileSearchSettings"` — не рендерятся до загрузки.

**Prevention.** Loading / empty / error — три разных состояния, рендерить нужно по три разные ветки. Бинарная логика «есть данные / нет данных» путает «грузится» с «пусто».

### Финальная проверка

```powershell
cargo test -p kepler-backend --lib file_index::
bun run --cwd shell typecheck
bun run ark:guard:writes
bun run docs:check
```

Результат: **file_index 27/27 PASS**, shell tsc clean, ARK write boundary guard passed, docs fresh.

### Третий проход — добиваем L1–L7

### L1 — env-race в `default_roots_tests`

**Root cause.** `cargo test` параллелизит тесты внутри одного бинаря. Тесты `env_override_takes_precedence` и `windows_default_returns_user_profile` мутируют `KEPLER_FILE_INDEX_ROOTS` и `KOSMOS_TEST_MODE`. Если запускаются одновременно — flake: один считывает чужой setup.

**Fix.** `static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());` в `default_roots_tests`. Каждый env-мутирующий тест берёт `_guard = ENV_LOCK.lock()` в начале.

**Prevention.** Process-level state (env, cwd, signal handlers) — implicit shared resource; ВСЕГДА сериализовать ручным lock'ом или crate'ом `serial_test`.

### L2 — `tmp`/`cache`/`out` блокировали legit user folders

**Root cause.** `NOISY_FOLDER_NAMES` содержал bare `tmp`, `cache`, `out`. `path_contains_noisy_folder` блокировал любой component с этим именем — `D:\projects\my-app\tmp-output`, `D:\projects\cache-libs`, `D:\projects\build-out`. Юзер не мог найти артефакты.

**Fix.** Удалены `tmp`, `cache`, `out`. Системный шум продолжает фильтроваться через DEFAULT_IGNORE_PATTERNS (`*.tmp`, `*.temp`, `**/Cache/**`, `**/AppData/**`) — там точные patterns, не bare names.

**Prevention.** Component-name filters — для однозначных artefact-папок (`node_modules`, `target`, `.git`). Общеупотребительные слова (`tmp`, `cache`, `out`, `bin`) — пользовательский конфликт неизбежен, лучше pattern-level filter.

### L3 — `remove_tree` блокировал writer mutex на весь DELETE

**Root cause.** Удаление 100k+ строк из `files` + `file_search_fts` (с trigram FTS index это O(N · log N) ребилда) — одна транзакция держит `Mutex<Connection>`. WS-driven settings writes блокируются на всё время (плюс `read_conn` имеет 5s busy_timeout — но писатели мьютекса игнорируют sqlite busy_timeout).

**Fix.** Chunked DELETE: цикл с `DELETE ... WHERE path IN (SELECT path WHERE <cond> LIMIT 5000)`, commit + drop(conn) после каждого chunk, `std::thread::yield_now()` между итерациями. Mutex держится в пределах 5k rows.

**Prevention.** Любое массовое DML в SQLite через `Mutex<Connection>` — chunked. Single big transaction = mutex hold time = blocked readers/writers.

### L4 — NTFS pipe `read_line` хрупкий

**Root cause.** `BufReader::read_line` останавливается на `\n` и ограничен буфером (8K default). Большой JSON response от service или multi-line формат сломает парсинг.

**Fix.** `pipe.read_to_string(&mut raw)` слурпает до EOF. Контракт остаётся — service закрывает write side после JSON-write.

**Prevention.** Если протокол на pipe message-oriented (один запрос → один ответ), `read_to_string` до EOF надёжнее line-delimited. Для streaming — length-prefix или explicit delimiter (рекомендация на будущее).

### L5 — A11y: semantic list + aria-label на chips

**Root cause.** Chips рендерились как `<div>`s; screen reader не объявлял «список из N»; кнопки «Удалить» без `aria-label` повторяли «Удалить, кнопка» N раз без указания на какой scope/pattern.

**Fix.** `<ul role="list">` + `<li>` per chip; `:aria-label="`Удалить папку поиска ${root}`"` / `:aria-label="`Удалить шаблон ${pattern}`"`.

**Prevention.** Chip-lists всегда `<ul>/<li>`; action-кнопки в списках — `aria-label` с уникальным контекстом, не голый текст.

### L6 — двойной `aria-live="polite"`

**Root cause.** ToastHost — wrapper с `aria-live="polite"`. Toast — ребёнок с тем же `aria-live="polite"`. Когда ребёнок появляется внутри live-region, и сам ребёнок — live-region, screen reader озвучивает дважды.

**Fix.** Убран `aria-live` с Toast. `role="status"` оставлен (семантика). ToastHost остаётся единственным live-region'ом.

**Prevention.** `aria-live` — на КОНТЕЙНЕРЕ, не на каждом ребёнке. Live regions не вкладываются.

### L7 — `mtime` failure тихо превращается в epoch 0

**Root cause.** `entry.metadata().ok().and_then(...).unwrap_or_default()` — если `metadata()` упал (permission denied на файле), `unwrap_or_default()` для `i64` = 0 (1970-01-01). Файл попадает в индекс с «древним» mtime, ломает будущие «recent files» sort'ы.

**Fix.** Match с tracing::trace! на error path: видим в логах какие файлы упали по метадате. Возвращаемое значение остаётся 0 (UI-совместимость), но теперь не silent.

**Prevention.** `unwrap_or_default()` для числовых типов хоронит ошибки за «правдоподобным» дефолтом. Явный match с log — даже trace-level — спасает диагностику.

### Финальная проверка третьего прохода

```powershell
cargo test -p kepler-backend --lib file_index::
bun run --cwd shell typecheck
bun run ark:guard:writes
bun run docs:check
```

Результат: **file_index 27/27 PASS**, shell tsc clean, ARK write boundary guard passed, docs fresh.

### Чеклист ручной проверки (проверить визуально в dev сборке)

После любой substantial-задачи build/typecheck не доказывает корректность UI — нужен визуальный smoke. Что проверить:

1. **Settings → Поиск файлов** открывается без consoleошибок, видна секция «Папки поиска».
2. **Добавление scope через folder picker** работает: native dialog, выбранная папка появляется в списке, появляется progress toast «Папка добавлена…».
3. **Добавление scope = `D:` или `C:\`** показывает confirm-dialog с предупреждением о масштабе. Cancel — не добавляется.
4. **Добавление UNC** (`\\share\path`) — отклоняется с понятной русской ошибкой.
5. **Удаление scope** показывает `window.confirm` с текстом «Все её проиндексированные файлы будут удалены». Cancel — не удаляется.
6. **Список scopes** под `<ul>`: tab-навигация переходит по каждой кнопке «Удалить»; screen reader (NVDA / VoiceOver Win) озвучивает «список из N элементов, Удалить папку поиска D:\Projects».
7. **Добавление шаблона** `*.tmp`, повторное `*.TMP` — показывает inline error «Шаблон уже добавлен: \*.TMP» (frontend-side, мгновенно).
8. **Добавление шаблона** `[abc` (invalid glob) — backend возвращает «invalid ignore pattern», виден в error banner.
9. **Заголовок секции** написан как «Шаблоны исключений» (не «Ignore patterns»). Toast после add: «Шаблон добавлен, индекс обновляется».
10. **Toggle «Исключать шумные папки»** — клик мгновенно меняет визуально (optimistic); если backend упадёт — возвращается обратно (не наблюдается без принудительной ошибки).
11. **Toggle «Учитывать `.gitignore`» + Toggle «Ускоренный NTFS-режим»** включены оба — статус под NTFS-toggle показывает «резервный режим». Hint объясняет почему.
12. **NTFS-toggle включён, диск-root в scopes** — после rescan статус показывает «активен» (Windows + admin) или «недоступен» (без admin) с понятной отсылкой.
13. **Кнопка «Переиндексировать»** disabled когда `scan_in_progress` истинно (видно по progress toast). Текст «Идёт…».
14. **Initial load**: при первом открытии Settings → Поиск файлов до загрузки видна строка «Загрузка настроек поиска…», не misleading «Папки не выбраны».
15. **Тосты прогресса не стэкаются**: при быстрых add → remove → add старые toast'ы dismiss'аются, актуальный один.
16. **Toast duration**: success-toast (зелёный) исчезает через ~2.6s, не раньше. Не «фантомно» дёрнулся раньше.
17. **Watcher после удаления scope**: trogger создать файл в удалённой папке — он НЕ появляется в search index (regression C1).
18. **Hidden NTFS файлы** (например `ntuser.dat` в Users) — не индексируются при `include_hidden=false`. Через ToolTip/Properties Explorer'а можно проверить что bit стоит.
19. **Под `KOSMOS_HEADLESS=1`** e2e не зависают на `pickScope` — handler возвращает null.
20. **`Settings → Поиск файлов`** на Sky screenreader (NVDA, Win Narrator) озвучивает каждую кнопку «Удалить» с конкретным scope/pattern, не голым «Удалить».
21. **AppData не индексируется** даже при «Исключать шумные папки» OFF — раньше регрешн (M1). Проверить: добавить `%USERPROFILE%` как scope, выключить toggle, переиндексировать, search «AppData» — пусто.
22. **Юзер-папка `D:\projects\my-app\tmp-output`** теперь индексируется (regression L2). Проверить: добавить `D:\projects\my-app\tmp-output` как scope, файлы внутри видны в search.

Если что-то из чеклиста не работает — это регрешн на одном из 22 fix'ов; смотри соответствующий пункт в evidence.md.

## Follow-up fix — removing a tracked folder waited on FTS cleanup

**PASS.** Manual testing exposed that deleting a tracked folder could still feel slow even after `scan_lock` was fixed. Root cause: `scope_remove` removed the root record and then synchronously deleted every matching row from `files` and `file_search_fts`; for `D:\` that can mean hundreds of thousands of rows and an expensive FTS delete. `FileIndex::remove_root` now removes the scope record immediately, restarts the watcher, schedules subtree cleanup via `spawn_blocking`, and starts background rescan. Search consistency is restored by background cleanup/rescan, while the Settings action returns quickly.

Regression:

- `services/kepler-backend/src/file_index/mod.rs::tests::scope_remove_does_not_cleanup_large_index_synchronously`
- `services/kepler-backend/src/file_index/mod.rs::tests::removing_root_hides_scope_immediately_and_cleans_index_in_background`

Command:

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index::
```

Result: backend file_index 18 passed, 0 failed.

## Follow-up UX — real indexing progress toast

**PASS.** File search settings now expose `scan_progress` with phase, root, roots done/total, files seen/indexed and message. `SettingsView` uses `@kosmos/visuals` toast host instead of an inline rectangle: one persistent toast is updated while indexing runs, showing current root and file count, then turns into success. Existing toast API remains backward-compatible and now supports update/loading/title/description.

Commands:

```powershell
cargo test --manifest-path services\kepler-backend\Cargo.toml --lib file_index::
bun run --cwd shell typecheck
```

Result: backend file_index 17 passed, shell typecheck passed.
