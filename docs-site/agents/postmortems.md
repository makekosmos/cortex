# Постмортемы багов

::: tip Зачем эта страница
Журнал реальных багов, которые уже починили. Каждая запись = **что сломалось**, **почему**, **как починили**, **как не допустить повторения**. Цель — чтобы будущий ты (или следующий агент) не повторял одни и те же ошибки.

Workflow ведения постмортемов — `bug-postmortem` skill (`.claude/skills/bug-postmortem/SKILL.md`).
:::

## Шаблон записи

```
## YYYY-MM-DD — короткое название

**Симптомы** — что наблюдал пользователь (1-2 предложения).
**Где жило** — `file:line` ссылки.
**Root cause** — почему именно это произошло, на уровне «не угадаешь без stack trace».
**Fix** — что изменили + commit hash (когда появится).
**Регрешн-защита** — какой тест ловит повторение.
**Prevention** — общий вывод: на что обращать внимание в похожих ситуациях.
```

---

## 2026-05-26 — Kosmos System Service uninstall оставляет legacy service

**Симптомы.** После миграции `KeplerFocusSvc` → `KosmosSystemSvc` Settings мог показывать системный сервис как установленный даже после успешного uninstall.

**Где жило.** `services/kepler-focus-svc/src/cli.rs:159` — `uninstall()` открывал один “первый найденный” сервис через `open_installed_service`.

**Root cause.** Compatibility helper был корректен для `status/start/stop`, где нужен один active service, но был переиспользован для `uninstall`, где семантика другая: upgrade-машина может легально иметь оба сервиса одновременно. Приоритет new-first приводил к удалению только `KosmosSystemSvc`; legacy `KeplerFocusSvc` оставался в SCM и следующий `status()` снова видел installed=true.

**Fix.** `uninstall()` больше не использует “первый найденный” service helper. Он проходит по `[KosmosSystemSvc, KeplerFocusSvc]`, для каждого найденного сервиса делает best-effort stop и delete, а отсутствие одного из имён считает idempotent success.

**Регрешн-защита.** `cargo test -p kepler-focus-svc uninstall_targets_new_and_legacy_service_names` проверяет, что uninstall-план всегда включает новое и legacy имя сервиса.

**Prevention.** Compatibility fallback и cleanup — разные операции. Fallback обычно должен выбирать один active target, а cleanup/migration должен рассматривать все legacy targets как независимые хвосты, которые могут одновременно существовать после upgrade.

## 2026-05-23 — Kepler: singleton conflict из-за pid reuse

**Симптомы.** После некорректного завершения kepler-backend (panic, kill, BSOD) при следующем запуске `bun run --cwd shell dev` backend бесконечно падает на старте с `FATAL setup: singleton conflict via lock-file`. Supervisor уходит в respawn-loop (1s → 5s → 30s → 60s → 120s), shell показывает `ArkClient not ready (timeout)`, IPC `kepler:ark:request` валится. Помогает только ручное удаление `%APPDATA%\Kosmos\kepler.lock.json`.

**Где жило.** `services/kepler-backend/src/main.rs:182-189` — гейт перед `SingletonGuard::acquire`. Использовал `services/kepler-backend/src/lock_file.rs:204` (`read_if_alive`), который через `services/kepler-backend/src/lock_file.rs:223` (`is_pid_alive` на Win — `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, ...)`) проверял только «жив ли PID», без валидации, что это именно kepler-backend.

**Root cause.** Pid-based liveness check — фундаментально ненадёжный механизм определения «работает ли мой сервис». Windows переиспользует освободившиеся PID'ы, и наблюдаемый эпизод тому пример: PID 14852 был записан в lock-файл предыдущим kepler-backend'ом в 17:56 (упал без cleanup'а), к 23:05 Windows отдала этот же PID запускающемуся electron-shell'у. Гейт честно отвечал «PID 14852 жив» (electron действительно жив), и backend отказывался стартовать. Хуже того: `is_pid_alive` принципиально не может различить «мой мёртвый процесс / переиспользованный PID живой electron'а / случайный chrome / explorer».

Архитектурно это была защита-в-глубину к настоящему механизму singleton'а — `SingletonGuard` на `kepler-singleton.lock.db` (SQLite WAL `BEGIN IMMEDIATE`). Этот lock работает корректно: kernel держит file handle, при любой смерти процесса handle освобождается, pid reuse физически не может его сломать. Но pid-гейт стоял **до** настоящего lock'а и отказывал раньше, чем тот успевал дать авторитетный ответ. То есть «защита в глубину» оказалась strict superset: первый слой ловил то, что второй пропустил бы корректно. JSON-файл должен быть discovery-метаданными для shell (ws_port, auth_token), а не gate'ом — параллель с Postgres'овым `postmaster.pid` (метаданные) vs. `flock` на data directory (настоящий gate).

**Fix.** Pid-based гейт убран. Новый pub helper `kepler_backend::singleton::acquire_clearing_stale_lock(lock_path, singleton_path)` делает `SingletonGuard::acquire` (SQLite WAL `BEGIN IMMEDIATE` — OS-level file lock), затем безусловно удаляет stale `kepler.lock.json` если он был. JSON остаётся discovery-метаданными, перезаписывается `lock_file::write_atomic` после `ws.bind`. Удаление stale JSON **сразу после acquire** (а не лениво при write_atomic) закрывает race-окно «старый ws_port на диске пока новый WS ещё не bind'нулся» — shell, прочитавший JSON в это окно, получит `ENOENT` → retry в supervisor'е, а не connect к мёртвому endpoint'у с истёкшим auth token'ом. Мёртвый код `lock_file::read_if_alive` и `lock_file::is_pid_alive` (вместе с Win-веткой через `OpenProcess` и Unix-веткой через `kill(pid, 0)`) удалён — иначе через полгода кто-то «починит» обратно, не разобравшись.

**Регрешн-защита.** `services/kepler-backend/src/singleton.rs::tests`:

- `stale_lock_with_live_unrelated_pid_does_not_block_acquire` — пишет `kepler.lock.json` с `pid = std::process::id()` (это cargo test binary — гарантированно живой, гарантированно НЕ kepler-backend) и проверяет, что `acquire_clearing_stale_lock` возвращает `Ok` + reported_pid + удалил файл. Тест прямо симулирует наблюдаемый сценарий pid reuse.
- `acquire_clearing_stale_lock_handles_missing_json` — first-time startup, JSON отсутствует.
- `acquire_clearing_stale_lock_removes_corrupt_json` — частично записанный JSON от прерванного `write_atomic` тоже не блокирует.
- `second_acquire_clearing_stale_lock_fails_with_already_running` — параллельный second-acquire падает с `SingletonError::AlreadyRunning`, и сообщение об ошибке содержит «Kepler» (часть контракта — diagnosability в логах supervisor'а).

Manual repro для AC7: запустить `bun run --cwd shell dev`, дождаться bind, `Stop-Process` на kepler-backend.exe без cleanup'а, дождаться respawn — должен подняться.

**Prevention.**

- **PID — это не identity процесса.** Любой код, говорящий «PID жив → процесс X жив», содержит скрытое допущение «PID не переиспользован». Допущение ломается всегда — Windows крутит счётчик быстро, Linux обнуляет на 32767 по умолчанию. Если нужно «работает ли мой сервис», ответ один: **OS-level lock на файле/сокете/named pipe**, который kernel освобождает на смерть процесса.
- **Discovery-метаданные ≠ gate.** Postgres: `postmaster.pid` содержит port + socket dir, но startup gate — это `flock` на data directory. SQLite: `<db>-wal` хранит WAL state, lock — POSIX advisory range lock на самой DB. Любой файл, который один процесс пишет на старте а другие читают для discovery, **не может одновременно быть и gate'ом** — потому что читатель не может атомарно «прочитать + захватить». Если в коде появляется паттерн «прочитать какой-то файл, проверить какой-то признак, решить можно ли стартовать» — это red flag.
- **Defense-in-depth не освобождает от корректности первого слоя.** Здесь pid-гейт стоял **до** настоящего `SingletonGuard` и отказывал раньше. В результате «защита» оказалась strict liability — первый слой ловил ровно те случаи, которые второй обработал бы корректно. Прежде чем добавлять «дополнительную проверку», спросить: что именно она ловит, чего не ловит настоящий механизм, и какой её false-positive rate? Если правильный слой работает — лишний только увеличивает поверхность отказа.
- **Lock-файл должен исчезать после graceful shutdown.** Уже есть (`main.rs:143` — `remove_file(&lock_path)` в shutdown path), но ungrateful shutdown оставит JSON на диске; новый код к этому resilient (удаляет stale при следующем старте). Если в проекте появятся другие discovery-файлы — same rule.

**Связанные правила.** [forbidden.md § Rust](/agents/forbidden) — Mutex poison recovery + `RUST_BACKTRACE=1` + `db_backup`. Этот случай — пример того, что singleton-механизмы должны быть на kernel-уровне; pid-based — анти-паттерн.

---

## 2026-05-23 — Kepler: file search показывает только файлы из %APPDATA% (ИСПРАВЛЕНО 2026-05-24)

**Симптомы.** File search в launcher находит только файлы под `%APPDATA%\Roaming\...`, не находит проекты на `C:\` / `D:\`.

**Где жило.** Гипотеза по `services/kepler-backend/src/file_index/scanner/ntfs.rs:13-29` (`scan_drive_root`) → service pipe fail → `Volume::new(\\.\C:)` fail (admin required) → `services/kepler-backend/src/file_index/scanner.rs:70` (`scan_walk_root`) fallback. Default roots в `scanner::default_roots()` **корректны** — `GetLogicalDrives` + `GetDriveTypeW`, lock_dir НЕ передаётся как root.

**Root cause.** Архитектурная хрупкость — два exclusive fast-path (focus-svc named pipe / direct NTFS USN) **оба требуют privilege** (installed Windows service / admin token). Fallback на user-space `WalkDir` от C:\ настолько медленный что initial rescan не выходит за пределы системных каталогов за обозримое время; пользователь видит первые indexed файлы (которые случайно в `C:\Users\<u>\AppData\` потому что walk идёт alphabetically/depth-first из C:\) и считает что indexer работает только в AppData.

**Fix.** Substantial-задача `.agent/tasks/2026-05-24-file-search-scopes-ntfs/`: default roots заменены с fixed drives на `%USERPROFILE%`; scopes теперь persist'ятся в `file-index.db` (`file_index_roots`) и управляются из Settings → Поиск файлов. User-mode scanner получил root-relative hardcoded ignores, user-configurable ignore patterns, `.gitignore` toggle, hidden-files toggle. Existing NTFS/MFT fast path стал честным opt-in через настройку «Ускоренный NTFS-режим»: OFF гарантирует user-mode scan, ON пробует NTFS только когда применимо и fallback'ится на user-mode. `settings_set` стал partial patch API, добавлены `scope_add/scope_remove/ignore_add/ignore_remove`.

**Регрешн-защита.** `cargo test -p kepler-backend file_index::`:

- `scanner::default_roots_tests::windows_default_returns_user_profile` — default root больше не full-drive scan.
- `file_index::tests::scan_searches_regular_files_and_skips_noisy_folders_by_default` — обычные файлы индексируются, noisy folders пропускаются.
- `file_index::tests::ignore_patterns_filter_matching_files` — пользовательский `*.tmp` отсекает файлы.
- `file_index::tests::removing_root_hides_scope_immediately_and_cleans_index_in_background` — удаление scope сразу убирает root из настроек и затем очищает subtree в фоне.
- `file_index::tests::scope_remove_does_not_cleanup_large_index_synchronously` — удаление большого scope не ждёт синхронный FTS/files cleanup.
- `store::tests::roots_and_ignore_patterns_are_persisted_and_deduped` — scopes/patterns persist + dedupe.

**Prevention.** **Privilege-dependent fast path не должен быть default UX-контрактом.** Default должен быть медленнее, но гарантированно рабочим без admin, а ускорение — явным opt-in с fallback. Для filesystem traversal noisy-folder filters должны считаться относительно search scope, а не абсолютного пути: иначе тестовые/temp roots под `%USERPROFILE%\AppData\...` сами себя отфильтруют и дадут ложное ощущение «индексатор пустой». Любой backend setting, который меняет roots, обязан либо перезапускать watcher, либо явно инвалидировать его — stale watcher по старым roots создаёт рассинхрон между Settings и результатами поиска. Root/scope mutation не должна синхронно делать bulk cleanup по FTS-таблице: пользовательское действие должно менять конфигурацию сразу, а тяжёлую чистку/полную переиндексацию выполнять как background job с прогрессом.

### Follow-up audit pass 2026-05-24 — 12 багов в Phase 1

После реализации Phase 1 трёхвекторный audit (backend Rust / shell UI / cross-cutting) выявил 12 серьёзных проблем. Они зафикшены тем же днём:

- **C1 watcher не фильтровал события удалённых scopes** → stale upsert после `remove_root`. `notify`-канал содержит pre-restart события; `apply_path` ловит их и переиндексирует «удалённое». Fix: `path_is_under_any_root` filter в `handle_event`, case-insensitive prefix check без false-prefix-match. Regression: `watcher::tests::events_outside_active_roots_are_dropped`.
- **C2 NTFS-hidden attribute игнорировался** → `AppData`, `Thumbs.db`, `ntuser.dat` индексировались на NTFS fast path несмотря на `include_hidden=false`. `is_dot_hidden` смотрит только дот-префикс, что POSIX-only. Fix: `path_has_hidden_attribute` через `MetadataExt::file_attributes()` проверяет HIDDEN|SYSTEM. Regression: `scanner::tests::ntfs_hidden_attribute_excludes_file_when_include_hidden_off`.
- **H2 NTFS fast path не активировался для picker-вариантов** → `D:`, `D:\\`, `d:/` мимо byte-length проверки. Fix: trim + 2-byte canonical. Regression: `drive_root_detection_accepts_picker_variants`.
- **H4 SQLITE_BUSY на settings_get во время rescan** → reader без `busy_timeout` падал сразу. Fix: `busy_timeout(5s)` в `read_conn`.
- **H5 удаление scope без confirm** → необратимая фоновая чистка тысяч строк по одному клику. Fix: `window.confirm` с явным текстом.
- **H6 английский в UI** («Ignore patterns», «Pattern добавлен») — нарушение жёсткого правила CLAUDE.md. Fix: «Шаблоны исключений», «Шаблон добавлен/удалён/уже добавлен».
- **H7 backend errors глоталась** → `console.warn(err)` + generic toast. Юзер не видел «pattern уже есть» vs «invalid glob». Fix: `describeFileSearchError` распознаёт known маркеры + frontend-side dedup до отправки.
- **M3/M4 case-insensitive dedup** → `*.tmp`/`*.TMP` и `D:\Personal`/`d:\personal\` коэкзистили из-за case-sensitive PRIMARY KEY. Fix: explicit `lower()` lookup + `normalize_root` (trim trailing slashes кроме drive root). Regression: `ignore_pattern_dedup_is_case_insensitive`, `roots_dedup_is_case_and_trailing_slash_insensitive`.
- **M5 `useToast.update({ duration })` timer leak** → новый `setTimeout` не отменял старый. Fix: `Map<id, timerId>` + cancel-before-schedule.
- **M7 «Переиндексировать» не учитывала server `scan_in_progress`** → дубль-rescan jobs в очереди на `scan_lock`. Fix: `:disabled` на server-side flag.
- **M9 native dialog hang под `KOSMOS_HEADLESS=1`** → e2e зависнет. Fix: early `return null` если headless.
- **M10 `validate_ignore_pattern` пропускал invalid globs** (`Glob::new` parses; `GlobSetBuilder.build` fails). Fix: полный pipeline в validate. Regression: `validate_ignore_pattern_rejects_unbuildable_globs`.

**Финальная проверка.** `cargo test -p kepler-backend --lib file_index::` → 25/25, `bun run --cwd shell typecheck` → clean, `bun run ark:guard:writes` → PASS.

**Prevention для будущего.** (1) Watcher state и settings state — независимые подсистемы; на стыке всегда фильтрация по current snapshot. (2) Платформо-зависимые понятия (hidden, executable) — никогда не сводить к POSIX convention. (3) Path-as-key — всегда canonicalize в одной точке (на write), не «store as-given, compare with lower()». (4) Validation должна повторять production path в миниатюре, не только первый этап парсера. (5) Server-side state — source of truth для disabled-условий UI, не local UI flag. (6) Любой нативный dialog в `shell/electron/*` — headless-guard обязателен; правило в CLAUDE.md давно есть, забыт при добавлении новой команды. (7) Catch-блок — это «понять или показать сырое», никогда не `console.warn(err)` без проброса в UI. (8) SQLite `busy_timeout` в любом read connection multi-connection setup — defaults = 0ms = немедленный fail. (9) При scheduling-операциях с overridable timeout — всегда отменять предыдущий handle.

**Известно открытым (next pass).** C3 (rescan-vs-mutation atomicity — eventual consistency через retry-spawn даёт корректный final state, но окно для transient stale между check и replace_all открыто), H1 (NTFS fast-path не уважает `.gitignore`), H3 (toggle-storm spawn N rescans без debounce), H8 (`pickScope` без guard на root-drive/UNC), H9 (strict IPC validation), H10 (NTFS toggle status indicator), M1/M2/M6/M8 (UX cosmetics).

### Второй проход audit-фиксов (тот же день)

Все 10 проблем из «известно открытым» закрыты во второй итерации:

- **C3** — self-healing follow-up rescan после `replace_all` если generation сменился. Regression: `rescan_schedules_followup_when_generation_changes_during_write`.
- **H1** — NTFS fast-path skip'ается когда `respect_gitignore=true`, fallback на user-mode walk; статус сообщается через `NtfsStatus::Fallback`.
- **H3** — `AtomicBool rescan_pending` coalescing'ит overlapping `spawn_rescan` calls. 50 параллельных вызовов → 1 фактический rescan. Regression: `rescan_coalesces_overlapping_spawn_requests`.
- **H8** — `scope:add` IPC handler: UNC paths отклоняются, drive roots требуют confirm-dialog (headless bypass).
- **H9** — `settings:set` IPC handler: explicit allowlist boolean-полей. Любой ключ вне allowlist — игнорируется.
- **H10** — `NtfsStatus` enum в settings (`active|fallback|unavailable|disabled|unknown`); UI hint под toggle отображает реальный статус, не позицию переключателя.
- **M1** — DEFAULT*IGNORE_PATTERNS отделены от NOISY_FOLDER_NAMES. Paths applies всегда (AppData/Cache/*.tmp/\_.temp), folder-names гейтятся toggle'ом. AppData больше не индексируется при `exclude_noisy_folders=false`.
- **M2** — optimistic toggle update с rollback из snapshot при ошибке. Никаких visual flip-back.
- **M6** — `clearFileSearchPoll` теперь dismiss'ит progress toast (был sticky навсегда из-за `duration:0`).
- **M8** — loading state отдельно от empty: «Загрузка настроек поиска…» вместо misleading «Папки не выбраны».

**Prevention для второго прохода.** (1) Eventual consistency: если check не atomic с write — self-healing follow-up обязателен. (2) Конфликт feature-flag'ов — явный resolver, не silent bypass. (3) Debounce/coalesce — через atomic swap (lock-free), не через `Mutex<bool>`. (4) Любая операция «весь диск» / сетевой ресурс — pre-flight warning. (5) IPC payload от renderer — explicit allowlist, не spread `Record<string, unknown>`. (6) Silent-fallback переключатели должны иметь status indicator. (7) Не складывать разные концепции под один toggle. (8) Optimistic updates стандартны для бинарных toggle'ов. (9) Cleanup симметричен setup'у. (10) Loading / empty / error — три разных UI-state, не один.

Финальная проверка: backend 27/27 PASS, shell tsc clean, ARK write boundary guard PASS, docs:check PASS.

### Третий проход — добиваем L1–L7

Все «минорные» проблемы из третьей категории закрыты:

- **L1** Env-race в `default_roots_tests` — static Mutex сериализует тесты, мутирующие process env.
- **L2** `tmp`/`cache`/`out` удалены из NOISY_FOLDER_NAMES — слишком общие, ломали legit user folders. Системный шум защищён DEFAULT_IGNORE_PATTERNS (`*.tmp`, `**/Cache/**`, etc).
- **L3** `remove_tree` chunked: DELETE по 5000 строк, mutex отпускается между chunks. WS settings writes больше не виснут на длинных cleanup.
- **L4** NTFS pipe response: `read_to_string` до EOF вместо `read_line` — устойчиво к multi-line/большим JSON.
- **L5** A11y: `<ul role="list">` + `<li>` для chip-lists; `aria-label` на «Удалить»-кнопках с уникальным контекстом (имя scope/pattern).
- **L6** `aria-live` убран с Toast — оставлен только на ToastHost (live region не вкладываются, screen reader озвучивал дважды).
- **L7** `mtime` metadata-failure теперь tracing::trace вместо silent `unwrap_or_default()` — диагностируемо в логах.

**Prevention для третьего прохода.** (1) Process env / cwd / signal handlers — implicit shared resource; всегда сериализовать в тестах. (2) Component-name filters — для однозначных artefact-папок; bare common words (tmp, cache, out, bin) — pattern-level filter. (3) Mass DML в SQLite через `Mutex<Connection>` — обязательное chunking. (4) Message-oriented pipe protocols — `read_to_string` до EOF надёжнее line-delimited. (5) Chip-lists / repeated action buttons — semantic markup + контекстный `aria-label`. (6) `aria-live` живёт на контейнере, не на детях. (7) `unwrap_or_default()` для числовых типов хоронит ошибки; явный match с tracing.

**Чеклист ручной проверки** (22 пункта) — в `.agent/tasks/2026-05-24-file-search-scopes-ntfs/evidence.md` § «Чеклист ручной проверки». Содержит: confirm-dialog на drive root, UNC reject, optimistic toggle, NTFS status indicator, loading state, watcher cleanup, hidden NTFS files, screen reader narration по scope/pattern. Это substantive UI verification, который build/typecheck не покрывает.

Финальный результат: 22 fix'а в трёх итерациях, 0 нарушений AC, 27 regression-тестов, 4 зелёных гварда (`cargo test`, `tsc`, `ark:guard:writes`, `docs:check`).

---

## 2026-05-23 — Kepler: toggle автозапуска фейлится с «не удалось применить настройку»

**Симптомы.** Settings → переключатель «Автозапуск с Windows». Клик — UI показывает «Не удалось применить настройку», хотя запись физически появляется в `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

**Где жило.** `shell/electron/settings-window.ts:153` (`isAutostartEnabled`) + `:177` (verify-readback в `setAutostartEnabled`).

**Root cause.** Асимметричный Electron Windows API: `app.setLoginItemSettings({ path, args, openAtLogin })` пишет в HKCU значение `"<exe>" --autostart`. `app.getLoginItemSettings()` **без аргументов** читает HKCU и сравнивает с дефолтным `process.execPath` (без args) — mismatch → возвращает `openAtLogin: false`, хотя запись физически есть. Это документированное поведение Electron, но non-obvious: написал с `{path, args}` → должен и читать с `{path, args}`. Симптом «set успешен, get возвращает false → verify фейлится → UI показывает error».

**Fix.** Константа `AUTOSTART_ARGS = ["--autostart"]` единственный источник. И `isAutostartEnabled()`, и verify-readback внутри `setAutostartEnabled` передают `{ path: process.execPath, args: AUTOSTART_ARGS }` в `getLoginItemSettings`. Симметрично с `set`.

**Регрешн-защита.** Defense-in-depth: одна константа гарантирует что изменение `args` потребует менять оба места одновременно. E2e на HKCU непрактично (Playwright бежит в test slot где autorunEnabled=false) — manual repro: prod-сборка → Settings → toggle → перезагрузка → проверить состояние тоггла и HKCU.

**Prevention.** **Когда Electron-API имеет парный `set`/`get`, проверяй симметричность сигнатур сразу.** Любая запись в реестр с non-default arguments должна верифицироваться чтением **с теми же** arguments. Применимо везде где есть `app.getXxx()` / `app.setXxx()` с optional parameters — `setUserTasks`, `setJumpList`, `setLoginItemSettings`.

---

## 2026-05-23 — Kepler: launcher window не показывается при manual launch

**Симптомы.** При запуске `Kepler.exe` (через ярлык, exe, после установки NSIS, после autoupdater quit-and-install) launcher window не появляется — пользователь видит только tray icon, не понимает запустилось ли приложение вообще. (Пользователь жаловался на противоположный симптом — окно показывается при autostart — но фактический баг был симметричен: «всегда скрыто», т.е. оба пути сломаны.)

**Где жило.** `shell/electron/main.ts:1308-1310` в `app.whenReady().then(...)` — `createLauncher()` создавал окно с `show: false` и `launcherHidden = true`, но **ни одной ветки не было** которая бы дёргала `showLauncher()` при manual launch. Маркер `--autostart` уже выставлялся в `setAutostartEnabled` через args, но в `whenReady` им только логировали диагностику — поведение не зависело.

**Root cause.** Эволюционный артефакт. Изначально launcher был hidden-by-default + hotkey-only (стиль PowerToys Run / Spotlight). Но Kepler **также** имеет tray + меню запуска + ярлык — для пользователя, кликнувшего exe вручную, hidden-by-default UX-сломан (нет обратной связи). Маркер `--autostart` был добавлен для будущей дифференциации, но саму дифференциацию никто не дописал. Комментарий в `main.ts:1299` гласил «launcher по умолчанию hidden — это by design», что **закрепляло баг как намерение**.

**Fix.** Чистая функция `shouldShowLauncherOnStartup(argv): boolean` в `main.ts` — возвращает `!argv.includes("--autostart")`. После `createTray()` в `whenReady` вызываем `showLauncher()` если функция вернула true. Согласовано с `AUTOSTART_ARGS` в settings-window.ts (Bug «toggle автозапуска фейлится» fix зависит от того же маркера).

**Регрешн-защита.** `shouldShowLauncherOnStartup` — чистая функция от argv, экспортирована, тестируется без Electron mock'ов. Manual repro: `Kepler.exe` → окно показывается; `Kepler.exe --autostart` → tray-only.

**Prevention.** **Любая стартовая UX-логика разветвляющаяся по argv/env должна жить в маленькой чистой функции с явным test-surface** — иначе превращается в куски кода с шестью `if (process.env...)` ветками без проверок. Комментарии вида «by design» **без spec-ссылки** — красный флаг: если правило настоящее — оно в `docs-site/`, если нет — фоссилизированный артефакт, который маскирует bug как намерение.

---

## 2026-05-23 — Horologion: focus widget пропадает между фазами pomodoro

**Симптомы.** Pomodoro запущен, focus widget (docked корнер) показывается нормально, через ~25 минут (длительность work-фазы) **пропадает**. Pomodoro session при этом не stop'нута — `completed_pomodoros` уже инкрементнут, phase = ShortBreak (или LongBreak) с `isRunning=false`, ждёт ручного Skip/Start. Пользователь думает что pomodoro «всё ещё идёт», индикатора нет.

**Где жило.** `shell/electron/focus-widget.ts:446` (`deriveFocusStateFromBackend`, `widgetActive = isRunning && phase !== "idle"`) + `extensions/horologion/src/lib/usePomodoroSession.ts:147` (`pushFocusWidgetState`, `active = isRunning.value && !isPaused.value && phase.value !== "idle"`).

**Root cause.** Два разных контракта столкнулись. Backend session (`crates/ark-core/rust/src/pomodoro/session.rs:399-438`, `finish_phase`) при `auto_start_break=false` (default config) после work делает `is_running=false`, эмитит `Finished`, затем переключает phase на ShortBreak/LongBreak с `is_running=false` и эмитит `PhaseChanged`. Backend трактует «не idle» = «session жива» (включая межфазный простой); widget derive трактовал «visible» как «активно тикает». При корректном backend-state `phase=ShortBreak, isRunning=false` widget уходил в hide, потеряв связь с реально живущей session. Единственный валидный индикатор «session закончилась» — `phase === "idle"`, потому что **только** `Session::stop()` возвращает phase в Idle.

**Fix.** В обоих местах деривации (main process + renderer push) `widgetActive` теперь `phase !== "idle"`. `phaseEndsAtMs` остаётся `null` пока `isRunning && !isPaused` ложен — автономный tick widget'а не запустится при межфазном простое, виджет покажет статичный MM:SS = `remainingSec`.

**Регрешн-защита.** `tests/e2e/horologion-focus-widget-between-phases.spec.ts` (headless Playwright): start pomodoro через ARK op → assert widget active → skip (work→ShortBreak с isRunning=false) → assert widget **всё ещё** active и BrowserWindow жив → stop → assert widget inactive. Без зависимости от реального 25-минутного wait.

**Prevention.** **Источник правды о видимости UI-элемента должен быть один и совпадать с lifecycle сущности, а не с её sub-state'ом.** «Сессия жива» (session.is_alive() = phase !== Idle) ≠ «активно тикает» (is_running). Если деривация одного UI-state'а живёт в двух местах (main derive + renderer push) — оба должны использовать **одну** чистую функцию, а не дублировать формулу. Кандидат: вынести `deriveWidgetActive(phase, isRunning, isPaused)` в `@kosmos/ark` или общий util и импортировать в обоих сайтах. Применимо ко всем UI-элементам которые tied к long-running backend state'у — focus widget, sync indicator, dashboard, любой tray badge.

---

## 2026-05-23 — Eden: drag-select прыгает по viewport

**Симптомы.** Mouse drag-select в Eden — viewport резко прокручивается вверх/вниз даже когда курсор далеко от края окна. Обычный in-view drag-select становится практически невозможным.

**Где жило.** `extensions/eden/src/Editor.css:28-29` + дубль `extensions/eden/src/App.css:2283-2284` — `scroll-padding-top: 30vh; scroll-padding-bottom: 30vh` на `.editor-wrapper`.

**Root cause.** Chromium во время mouse-drag для text-selection периодически вызывает native `scrollIntoView` для selection endpoint'а. `scroll-padding` участвует в расчёте видимости — что в padding-зоне считается «невидимым». При `padding: 30vh` любая selection в верхних или нижних 30% viewport'а триггерила scroll-into-view → viewport «догонял» selection прыжками. Не воспроизводится в jsdom (нет реализации selection scroll), не отлавливается через JS stack trace (вызов — из native Chromium).

**Fix.** Удалить `scroll-padding-top/bottom` из обоих селекторов `.editor-wrapper`. Breathing room «снизу при письме» сохранён через существующий `padding-bottom: 50vh` на `.ProseMirror` — это реальный layout, не виртуальный через scroll-padding. Autoscroll при выходе курсора за пределы окна (native Chromium + `useBlockSelection` для rubber-band) продолжает работать.

**Регрешн-защита.** Чистый CSS-фикс — JS-тест не нужен. В `Editor.css` оставлен комментарий объясняющий почему НЕ возвращать `scroll-padding`. Manual repro: открыть длинную заметку, поставить курсор в центр, drag-select мышью в пределах viewport — viewport должен оставаться стабильным.

**Prevention.** `scroll-padding` >> 0 на скролл-контейнере, содержащем contentEditable / textarea — **анти-паттерн**. Любой UA-инициированный scrollIntoView (drag-select, IME composition, find-in-page, accessibility focus) будет двигать viewport относительно padding-зоны, не относительно реальных краёв. Если нужно breathing room — используй реальный `padding-bottom` / spacer элемент.

---

## 2026-05-23 — file_index UNIQUE constraint вешает backend

**Симптомы.** После часа работы Kepler внезапно подвисает: Eden показывает infinite loading, `commands.list` / `app_index.list_all` / `focus.*` все таймаутят через 30s. Backend.exe жив как процесс, supervisor не делает respawn, но WS-server не отвечает.

**Где жило.** `services/kepler-backend/src/file_index/store.rs::replace_all` (схема `files.path TEXT PRIMARY KEY` в `store.rs:14`). Вызов из `services/kepler-backend/src/file_index/mod.rs::rescan:102`, async-spawn из `services/kepler-backend/src/main.rs:299`.

**Root cause.** `replace_all` использовал простой `INSERT INTO files (path, ...) VALUES (?, ?, ?, ?)` без дедупа и без `OR REPLACE`. NTFS fast scan на C:\ + D:\ может вернуть один и тот же абсолютный path дважды:

- Junction points (`C:\Documents and Settings` → `C:\Users`)
- Symlinks (часто в `C:\ProgramData\Package Cache`)
- Mount points (D:\ как junction внутри C:\)
- WSL'овский `\\?\` namespace, видимый из двух root'ов

Первый дубликат → `UNIQUE constraint failed: files.path` → транзакция rollback → `Err` из `rescan` → лог пишет `WARN file_index initial rescan failed` (`main.rs:307`) и task завершается. **Сам fail не должен валить backend** — но в наблюдаемом инциденте после WARN backend-лог обрывается на час, потом начинают сыпаться 30s-таймауты на всех ark-операциях. Точная second-order причина (Windows hibernate vs. блокировка tokio worker через `std::sync::Mutex<Connection>` в долгой транзакции vs. шторм событий watcher'а от browser hang) не установлена однозначно из имеющихся логов — backend stderr capture не было.

**Fix.** [commit hash] — `replace_all` теперь дедупит paths через `HashSet<&str>` перед `INSERT`. Дедуп выбран вместо `INSERT OR REPLACE` потому что:

1. Дубликаты в input — **сигнал** что scanner возвращает мусор (полезно ловить в логах позже, если такой dedup-counter добавим).
2. `INSERT OR REPLACE` молча перезаписывал бы и при реальных багах upsert-логики.
3. FTS-индекс при дубликатах путей дал бы две записи и поиск возвращал бы файл дважды.

**Регрешн-защита.** `services/kepler-backend/src/file_index/store.rs::tests::replace_all_dedupes_duplicate_paths` — кидает в `replace_all` три записи с двумя одинаковыми path'ами, проверяет что `Ok` и в БД ровно 2 строки.

**Prevention.**

- **Любой массовый bulk-insert по сырым данным из внешнего источника (FS scanner, network discovery, third-party API) обязан дедупиться перед `INSERT`.** SQLite UNIQUE — это контракт, который ты обещаешь соблюсти, не реактивный валидатор.
- **`std::sync::Mutex<Connection>` в долгих транзакциях, вызываемых из async task'а — анти-паттерн.** При 700k вставках держит worker thread tokio. Лучше: `tokio::task::spawn_blocking` или отдельный thread с channel'ом. См. [`db-resilience.md`](/concepts/db-resilience).
- **Backend должен писать stderr в файл, не только stdout.** Если stdout пропадает (zombie pipe), мы теряем все следы. Сейчас `crash_reporter` ловит panic, но stuck-без-panic — нет. Возможный TODO: периодический heartbeat-лог.
- **Supervisor должен делать health-check WS, а не только pid-alive.** Сейчас pid жив → supervisor спит, даже если WS-server stuck.

**Связанные правила.** [forbidden.md § Rust](/agents/forbidden) — про Mutex poison recovery, `RUST_BACKTRACE=1`, и обязательный `db_backup::maybe_backup_on_startup`. Этот случай показывает что недостаточно — нужен ещё health-check.
