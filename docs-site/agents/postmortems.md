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

## 2026-06-08 — File index стартовал полным сканом профиля

**Симптомы** — при cold start backend создавал file index и сразу запускал initial rescan, который по умолчанию обходил весь `%USERPROFILE%`. На реальном профиле это могло включать Downloads, Desktop, Documents, dev-репозитории, кэши и окружения, создавая CPU/IO spike и latency для RPC.
**Где жило** — `platform/runtime/src/main.rs:267-318`, `platform/runtime/src/file_index/scanner.rs:476-493`, `platform/runtime/src/file_index/mod.rs:261-342`.
**Root cause** — file index был спроектирован как always-on capability: если `KEPLER_FILE_INDEX_ROOTS` не задан и это не test mode, `default_roots()` seed'ил `%USERPROFILE%`, а `main.rs` без kill switch запускал `index.rescan()` после записи lock-файла. Сам `rescan_locked()` был `async`, но внутри синхронно делал filesystem walk и `replace_all` в SQLite, поэтому тяжелая работа занимала tokio worker runtime'а вместо blocking pool. `scan_generation` проверялся только после полного walk, так что изменение настроек во время большого scan не останавливало уже начатый обход.
**Fix** — `scanner::default_roots()` теперь возвращает пустой список без `KEPLER_FILE_INDEX_ROOTS`, поэтому `%USERPROFILE%` больше не seed'ится как implicit scope. В startup добавлены `KEPLER_FILE_INDEX=0` и `KEPLER_FILE_INDEX_INITIAL_RESCAN=0`; initial rescan стартует только когда index включён, flag initial rescan включён и roots непустые. `FileIndex::new_disabled()` даёт настоящий kill switch: search/settings/rescan API остаются безопасными, но roots/files не экспонируются и background scan не запускается. `rescan_locked()` выносит filesystem walk и SQLite `replace_all` в `tokio::task::spawn_blocking`, а scanner принимает cancellation predicate и проверяет его внутри walk loop до commit.
**Регрешн-защита** — `cargo test -p kepler-backend file_index` (через `CARGO_TARGET_DIR=.tmp/cargo-file-index-test`) проверяет пустые default roots без opt-in, disabled safe surface, env flag parser, cancellation внутри scanner loop и прежние file index контракты. `ark:smoke` (через `CARGO_TARGET_DIR=.tmp/cargo-ark-smoke`) прошёл полностью, включая ARK write boundary guard и kepler-backend Rust tests.
**Prevention** — Desktop background indexers не должны иметь broad filesystem roots или startup scan как implicit default. Любая capability, которая может обойти user profile / drive / repo tree, обязана иметь отдельные kill switches для всей функции и для startup work, пустой default до opt-in, blocking isolation для FS/SQLite jobs и cooperative cancellation внутри long loop, а не только stale-result discard после завершения.

## 2026-06-08 — Скрытый launcher бесконечно искал файлы

**Симптомы** — после ввода короткого query в launcher и скрытия окна backend мог продолжать получать `file_index.search` каждые ~800 мс. На большом file index это выглядело как периодический или постоянный CPU burn у `kepler-backend`.
**Где жило** — `platform/desktop/src/views/LauncherView.vue::scheduleFileSearch`, `platform/desktop/electron/main.ts::hideLauncher`, `platform/desktop/electron/preload.ts::api.window`, `platform/runtime/src/file_index/store.rs::search`.
**Root cause** — renderer реализовал file search как self-refresh loop: успешный `file_index.search` снова планировал тот же query через 800 мс. Launcher window при закрытии/blur не уничтожается, а скрывается через `BrowserWindow.hide()`, поэтому `onUnmounted()` не срабатывает и timer живёт дальше. Отдельно backend для query короче 3 символов обходил FTS и делал substring `LIKE '%q%'` по `name/path`, то есть скрытый polling по `a`/`do` превращался в повторяющийся scan по индексу файлов.
**Fix** — `LauncherView.vue` больше не self-reschedule'ит file search после результата: поиск запускается только debounce'ом от изменения query. Добавлен `cancelFileSearch()` и IPC событие `kepler:window:hide` (`main.ts` → `preload.ts` → renderer), которое инвалидирует текущий run, чистит timer и очищает file-команды при скрытии окна. Backend `FileStore::search` теперь возвращает пустой результат для query короче 3 символов и больше не имеет substring `LIKE '%q%'` fallback. `backgroundThrottling` у launcher'а оставлен выключенным только на macOS, где это нужно для occlusion workaround; на Windows/Linux он снова включён.
**Регрешн-защита** — `tests/unit/launcher-file-search-contract.test.ts` проверяет отсутствие self-refresh polling, наличие hide bridge/cancel path и платформенный `backgroundThrottling`. `cargo test -p kepler-backend short_queries_do_not_scan_files_table` проверяет, что 1-2 символа не возвращают file-index результаты, а 3+ символа продолжают работать. `ark:smoke` дополнительно поймал старый file-index тест с 1-character query; assertion обновлён на 3+ символа.
**Prevention** — Hidden/long-lived Electron windows нельзя полагаться на `onUnmounted()` для остановки side effects: каждый show/hide lifecycle должен иметь явный renderer event и cleanup для timers/subscriptions. Поиск в больших локальных индексах должен иметь минимальную длину query или dedicated prefix-index; substring scan по 1-2 символам нельзя оставлять за hot UI input path.

## 2026-06-07 — macOS: смена хоткея в настройках не реагирует на клавиши

**Симптомы** — в Настройках при клике на поле захвата хоткея появляется «Нажми сочетание…», но нажатия клавиш не регистрируются — сочетание не меняется. На Windows тот же UI работает.
**Где жило** — `packages/visuals/components/HotkeyCapture.vue::start` / `onKey`.
**Root cause** — компонент ловит хоткей через DOM `@keydown` на `<button>`, что требует фокуса на этом элементе. На macOS клик по `<button>` по умолчанию **не передаёт ему focus** (WebKit/Chromium следует системной настройке Full Keyboard Access — в отличие от Windows/Linux, где button фокусится по клику). Без фокуса keydown уходит мимо button → `onKey` (с гардом `if (!capturing.value) return` плюс отсутствие события) не отрабатывает → капчур висит в состоянии ожидания.
**Fix** — в `start()` добавлен программный `buttonRef.value?.focus()` (программный focus работает на всех платформах). Заодно отображение модификаторов сделано платформо-зависимым: на macOS — нативные символы `⌘ ⌥ ⌃ ⇧` (через `navigator.platform` детект), на остальных — текст `Win/Alt/Ctrl/Shift`.
**Регрешн-защита** — визуальная проверка на macOS: Настройки → клик по полю хоткея → нажать сочетание → оно отображается и сохраняется.
**Prevention** — Любой UI-primitive, ловящий keyboard-события через фокус-зависимый DOM-listener (`<button>`, `<div tabindex>`), на macOS должен явно вызывать `.focus()` при активации: клик не гарантирует focus на macOS. Не полагаться на «клик → элемент сфокусирован» как на кросс-платформенный инвариант.

## 2026-06-07 — macOS Shell: рендер замерзает через 1-2с + пустой экран в dev

**Симптомы** — (1) окно launcher'а открывается по хоткею, рендерится 1-2 секунды (можно ввести пару букв), затем визуально замерзает: ввод продолжает обрабатываться (при переоткрытии виден весь набранный запрос), но экран не перерисовывается. (2) В dev после нескольких перезапусков — стабильно пустой тёмный экран.
**Где жило** — `platform/desktop/electron/main.ts::createLauncher` (webPreferences окна, `openDevTools`), `showLauncher`; dev-артефакт — взаимодействие vite-plugin-electron с `app.requestSingleInstanceLock()`.
**Root cause** — (1) **macOS window occlusion throttling**: launcher — frameless окно с полупрозрачным `backgroundColor: "#00000000"` без постоянного always-on-top (на macOS его убрали из-за focus war). macOS Window Server помечает такое окно как occluded, и Chromium останавливает compositor для экономии — paint замерзает после первого кадра, хотя Vue реактивность продолжает работать. (2) **Пустой экран в dev** — зомби-Electron от грязных перезапусков держал single-instance lock; новый Electron, спавнутый vite-plugin-electron, не мог взять lock → `app.quit()` exit(0); плагин интерпретировал это как «Electron closed» и убивал shell-vite (порт 5173); живой зомби-Electron оставался на `chrome-error://chromewebdata/` (renderer dev server мёртв) → пустой тёмный экран.
**Fix** — против occlusion throttling: `backgroundThrottling: false` в `webPreferences` + Chromium switch'и `--disable-backgrounding-occluded-windows` и `--disable-renderer-backgrounding` (только darwin, до `app.whenReady`). Дополнительно: `openDevTools({ mode: "detach", activate: false })` (DevTools не ворует фокус → не тригерит blur), убраны `app.focus({ steal: true })` и `mainWindow.moveTop()` из `showLauncher()` (двойной `activateIgnoringOtherApps` создавал focus war). Пустой экран в dev лечится чистым перезапуском без зомби-процессов (`pkill -9 -f "MacOS/Electron"` перед стартом).
**Регрешн-защита** — визуальная проверка: открыть по хоткею → печатать 5+ секунд → каждая буква появляется немедленно (рендер не замерзает); CDP-проба `document.getElementById('app').childElementCount > 0` и `location.href === 'http://localhost:5173/'` (не chrome-error).
**Prevention** — Frameless/полупрозрачные окна на macOS подвержены occlusion throttling: для always-visible launcher/overlay ставить `backgroundThrottling: false` + occlusion switch'и. В dev с vite-plugin-electron + single-instance-lock грязный kill оставляет зомби, держащий lock — перед перезапуском всегда полностью убивать дерево Electron, иначе новый instance молча выходит и dev server гибнет.

## 2026-06-06 — Shell dev повторно стартовал на чужих портах

**Симптомы** — повторный `bun run --cwd platform/desktop dev` при уже живом dev-run строил backend/extensions, потом shell Vite писал `Port 5173 is in use, trying another one...`, уезжал на `5174`, Akasha падала с `Port 5185 is already in use`, а shutdown мог допечатать шум вроде `ERROR: The process "<pid>" not found`.
**Где жило** — `platform/desktop/scripts/dev.mjs` и `platform/desktop/vite.config.mjs`.
**Root cause** — extension dev servers уже запускались со `--strictPort`, а shell renderer Vite не имел strict port contract. Dev orchestrator не делал preflight по портам, поэтому вторая dev-сессия начинала частично стартовать и доходила до Electron/backend вместо раннего понятного отказа.
**Fix** — `dev.mjs` до spawn children проверяет shell port `5173` и devPort'ы выбранных Vue extension'ов (`akasha` default-on, все extensions при `KEPLER_DEV_EXTENSIONS=1`). Если порт занят, скрипт выходит до запуска children и печатает список занятых endpoints. Shell Vite теперь закреплён на `127.0.0.1:5173` со `strictPort`.
**Регрешн-защита** — `node --check platform/desktop/scripts/dev.mjs`; ручной busy-port check через `node platform/desktop/scripts/dev.mjs` при живых `5173/5185`; clean `bun run --cwd platform/desktop dev` должен показывать `http://127.0.0.1:5173/` и `http://127.0.0.1:5185/` без fallback на `5174`.
**Prevention** — Dev orchestration не должен полагаться на Vite fallback ports. Любой port-bound dev target обязан либо строго занять ожидаемый порт, либо до запуска зависимых процессов объяснить, какой предыдущий listener надо остановить.

---

## 2026-06-06 — Backend жёг CPU usage heartbeat и file watcher'ом

**Симптомы** — в простое `kepler-backend.exe` держал ~0.9-1.5 ядра, а `ark-core-rpc.exe` ещё ~0.3-0.5 ядра. RAM при этом не росла, поэтому проблема выглядела как CPU loop, а не memory leak.
**Где жило** — `platform/runtime/src/usage_tracker/mod.rs::run`, `ActiveSession::accumulate`, `update_active_session`; `platform/runtime/src/file_index/watcher.rs::start`.
**Root cause** — было два независимых фоновых churn-источника. Usage tracker каждую секунду для каждой активной in-memory session делал `upsert_usage_session`, а для foreground sample ещё и `upsert_tracked_app`; каждый ARK upsert bump'ал `lan_sync.version_vector`, поэтому без смены окна создавался постоянный write/sync churn. После снижения ARK churn `kepler-backend` всё ещё держал почти целое ядро: live dev profile показал `ark-core-rpc` ~9%, но backend ~98%. Изолированный backend с пустыми file-index roots потреблял ~0.2%, значит остаточный CPU шёл из recursive file watcher на persisted broad roots (`roots=2`, включая drive/user scope), который обрабатывал ambient filesystem events и писал локальный `file-index.db`.
**Fix** — `ActiveSession` теперь копит heartbeat time in-memory и flush'ит `usage_session` в ARK не чаще чем раз в 60 секунд; `tracked_app.last_seen_at` сохраняется при завершении session, чтобы heartbeat не делал второй sync bump. Финальный flush при завершении session остаётся обязательным. `update_active_session` больше не пишет `tracked_app` на каждый стабильный foreground poll. Recursive file watcher стал opt-in через `KEPLER_FILE_INDEX_WATCHER=1`; по умолчанию file index обновляется startup/manual rescan'ом без постоянного notify-потока.
**Регрешн-защита** — `cargo test -p kepler-backend active_session_heartbeat_flush_is_rate_limited` проверяет, что 59 секунд poll'ов не разрешают heartbeat flush, 60-я секунда разрешает, а после flush счётчик сбрасывается. `recursive_watcher_is_opt_in` фиксирует, что recursive watcher не включается без явного env opt-in.
**Prevention** — Любой фоновой sampler должен разделять sampling cadence и persistence cadence. Poll каждую секунду допустим для in-memory state, но запись в синхронизируемое хранилище обязана быть event-driven или bounded heartbeat; иначе каждый “без изменений” poll становится sync write. Recursive filesystem watcher по широкому root'у (`drive`, `%USERPROFILE%`, workspace с build dirs) не может быть default-поведением desktop shell: это opt-in capability с явным performance budget.

---

## 2026-06-06 — Clipboard detail терял выбранную строку и источник

**Симптомы** — при навигации стрелками по истории буфера выделение уходило ниже видимой области списка, поэтому пользователь не видел текущую выбранную запись. В detail-панели `Источник` либо показывал фейковый `Kosmos`, либо не мог показать реальный source вроде ShareX.
**Где жило** — `platform/desktop/src/components/ClipboardQuickPanel.vue` selection rendering/detail metadata; `platform/desktop/electron/clipboard-history-store.ts` record methods.
**Root cause** — список рендерил `selected` class по индексу, но не синхронизировал DOM viewport с программным `selectedIndex`, поэтому keyboard navigation меняла состояние без `scrollIntoView`. Clipboard store уже имел поле `source` в `ClipboardHistoryItem`/hydrate, но write path (`record`, `recordImage`, `recordFile`) не принимал source metadata, так что UI либо врал hardcoded строкой, либо не имел данных для честного источника.
**Fix** — `ClipboardQuickPanel` держит refs строк и при изменении `selectedIndex` вызывает `scrollIntoView({ block: "nearest" })`, поэтому keyboard selection остаётся в видимой области без лишних прыжков. Detail metadata показывает `Источник` только при наличии `selectedItem.source`. Clipboard store теперь принимает source metadata в `record` / `recordImage` / `recordFile`, а main recorder best-effort определяет Windows clipboard owner process и associated icon при новом clipboard snapshot.
**Регрешн-защита** — `bun test tests/unit/clipboard-history-store.test.ts` проверяет сохранение `source`/`sourceIcon` для text/file/image entries; `bun run shell:typecheck`; `bun run lint`.
**Prevention** — Keyboard-driven selection в кастомных списках должен явно синхронизировать selected row с scroll container; hover/click поведения недостаточно. Metadata UI не должен hardcode'ить происхождение данных: если источник приходит с backend/store boundary, показывай его условно, а если источник ещё не определён — не заполняй поле декоративной догадкой.

---

## 2026-06-05 — Focus launcher показывал команды не по состоянию сессии

**Симптомы** — в launcher были видны непонятные raw focus-команды, а «Начать фокус» оставалась доступной даже во время уже запущенной сессии.
**Где жило** — `platform/desktop/src/lib/focusLauncherCommands.ts::buildFocusAwareCommands`, `platform/desktop/src/views/LauncherView.vue::displayCommands`, `platform/desktop/electron/focus-session.ts::completeFocusSession`.
**Root cause** — command bus отдавал статический набор focus action id без знания текущего `pomodoro.get_state`, а launcher рендерил их как обычные команды. В результате lifecycle-команды не были связаны с active/paused состоянием и не заменяли стартовую команду.
**Fix** — добавлен state-aware helper, который в idle оставляет только «Начать фокус», а в active-сессии подставляет понятные команды «Приостановить/Продолжить», «Отметить задачу выполненной», «Завершить», «Редактировать». Launcher подписан на `focusSession.onUpdated`, а «Выполнена» идёт через `focusSession.complete()` и `upsert_object` для привязанной `task_obj`.
**Регрешн-защита** — `bun test tests/unit/focus-launcher-commands.test.ts`; `bun run shell:typecheck`; `bun run ark:guard:writes`; visual screenshots `.tmp/visual/2026-06-05-focus-launcher-commands/{launcher-idle.png,launcher-active-running.png,launcher-active-paused.png}`.
**Prevention** — Команды lifecycle для stateful Shell-сущностей нельзя показывать как плоский статический command bus список. Перед рендером launcher должен нормализовать такие команды через snapshot текущего состояния и скрывать действия, которые в этом состоянии бессмысленны.

---

## 2026-06-05 — LauncherView падал на обновлении списка команд

**Симптомы** — после `refreshCommands()` Vue логировал `Unhandled error during execution of component update`, затем падал с `TypeError: Cannot set properties of null (setting '__vnode')`.
**Где жило** — `platform/desktop/src/views/LauncherView.vue::refreshCommands`, template `v-for :key="cmd.id"` для списков команд.
**Root cause** — `refreshCommands()` напрямую мержил `commands.list()` и `app_index.list_all()` в reactive `commands.value`. Если источники возвращали повторяющийся command id, renderer получал duplicate keys в одном `v-for`, и Vue patcher мог упасть не на пользовательском коде, а глубоко в runtime при component update.
**Fix** — добавлен `dedupeCommandsById(...)`; `LauncherView::refreshCommands` теперь дедупит merged commands по id перед записью в `commands.value`.
**Регрешн-защита** — `bun test tests/unit/launcher-commands.test.ts tests/unit/focus-command-payload.test.ts tests/unit/focus-app-blocking.test.ts`; `bun run shell:typecheck`.
**Prevention** — Любой renderer list с `v-for :key` обязан получать уже нормализованный набор уникальных keys на data boundary. Если список мержится из нескольких источников (command bus + scanner + recents), дедуп делается до записи в reactive state, а не в template.

---

## 2026-06-05 — Focus app blocking выбрал не тот source of truth

**Симптомы** — в поле «Блокировка» пользователю предлагались exe/process-like entries с путями, а не те приложения/игры, которые он реально видит и запускает через Shell по хоткею. Выбранные элементы плохо совпадали с фактическим launcher entry, task mention иногда не выбирался/залипал, а mention popover на тёмной теме имел неправильный светлый shadow.
**Где жило** — `platform/desktop/src/components/FocusCommandPanel.vue::fetchApps`, `FocusCommandPanel.vue::pickTask`, `FocusCommandPanel.vue::.focus-command__mention`, `platform/desktop/src/views/LauncherView.vue::invokeSelected`.
**Root cause** — реализация взяла `app_index.list_all` как source of truth для Focus block suggestions, хотя продуктовый intent был «заблокировать то, что запускается из Shell launcher». `app_index` полезен для построения launcher entries, но сама блокировка должна работать на normalized launcher command identity и display model, иначе UI показывает технические exe-пути и расходится с тем, что пользователь выбирает в Shell.
**Fix** — Focus block payload теперь сохраняет `blockedApps` metadata (`id`, `name`, `icon`) рядом со старым `blockedAppIds`; backend active state хранит `blocked_apps`, а launcher отказывает запуск по exact id или normalized display name. Focus UI больше не показывает `exec_path` в suggestions, использует `@kosmos/visuals` `Textarea`, выбранная Delphi-задача и приложения рендерятся отдельными removable pills. Widget shield удалён. При blocked launch показывается overlay с 3-секундным hover-hold для разрешения на 5 минут.
**Регрешн-защита** — `bun test tests/unit/focus-command-payload.test.ts tests/unit/focus-app-blocking.test.ts tests/unit/focus-command-instant-render.test.ts`; `bun run shell:typecheck`; `$env:CARGO_TARGET_DIR='.tmp\cargo-focus-app-test'; cargo test -p kepler-backend set_then_get_active_state_round_trip`.
**Prevention** — Для UX, который должен совпадать с Shell launcher, source of truth обязан быть launcher-facing command identity/display model, а не raw backend scanner rows. Если backend scanner возвращает технические ids/paths, UI должен сохранять user-facing metadata и блокировать по стабильному id плюс normalized display name.

---

## 2026-06-05 — Focus command показывал прогрузку при входе

**Симптомы** — при входе в Shell-команду «Фокус» пользователь видел промежуточную загрузку, хотя это встроенная часть Shell и должна открываться моментально.
**Где жило** — `platform/desktop/src/components/FocusCommandPanel.vue::loading`, `FocusCommandPanel.vue::hydrate`, template branch `v-if="loading"`.
**Root cause** — Focus command panel смешал first paint с async hydration: локальная форма имела валидные shell-дефолты (`25 минут`, пустая цель, без блокировок), но компонент всё равно ставил `loading=true` и полностью заменял UI на `Загружаю фокус`, пока три IPC-запроса (`snapshot`, `listTasks`, `listBlocklists`) не завершатся. Для built-in Shell surface это превращало обычный переход между command pages в network-like loading state.
**Fix** — `FocusCommandPanel` больше не имеет blocking `loading` state: template всегда рендерит форму с локальными дефолтами, а `hydrate()` только фоном обновляет snapshot/tasks/blocklists и выставляет error при сбое IPC. Старый loading copy и CSS удалены.
**Регрешн-защита** — `bun test tests/unit/focus-command-instant-render.test.ts` проверяет, что SFC не содержит `v-if="loading"` и `Загружаю фокус`; visual verify `.tmp/visual/2026-06-05-focus-instant-open/focus-instant-720x460-immediate-final.png` открывает Focus command при искусственно задержанных IPC и проверяет, что `.focus-command__form` появляется за 500ms без loading-текста.
**Prevention** — Built-in Shell command pages не должны gate'ить first paint на async hydration, если у них есть валидные локальные defaults. IPC/ARK hydration может уточнять содержимое фоном, но shell navigation surface должен открываться сразу; loading-state допустим только для отсутствующего critical data, без которого нельзя показать осмысленный первый экран.

---

## 2026-06-05 — Shell TTL restore сбрасывал command page в список команд

**Симптомы** — если закрыть Shell на странице `Буфер обмена` или `Фокус`, повторное открытие в пределах TTL из Settings возвращало список команд вместо последней command page.
**Где жило** — `platform/desktop/src/views/LauncherView.vue::PersistedLauncherState`, `savePersistedState`, `window.kepler.window.onShow`.
**Root cause** — persisted launcher state сохранял только `query`, `selectedIndex`, `scrollTop`, `savedAt`, но не surface `mode`. На каждом `kepler:window:show` renderer безусловно делал `mode.value = "commands"`, поэтому TTL реально восстанавливал только поисковую строку и scroll, а не текущую Shell-позицию.
**Fix** — `PersistedLauncherState` получил `mode?: "commands" | "clipboard" | "focus"`, `savePersistedState()` пишет текущий mode, а show-handler восстанавливает fresh mode вместо безусловного сброса. `enterClipboardMode()` / `enterFocusMode()` сразу сохраняют state, чтобы закрытие Shell без дальнейшего scroll/input тоже переживало reopen.
**Регрешн-защита** — visual capture `.tmp/visual/2026-06-05-shell-command-surfaces-clipboard/capture.mjs` теперь вызывает synthetic `kepler:window:show` после открытия Focus и проверяет, что `.focus-command` остаётся на экране.
**Prevention** — TTL restore должен сохранять весь navigation surface, а не только локальное состояние списка. Любая новая Shell command page обязана либо участвовать в persisted `mode`, либо явно документировать, что она ephemeral и не восстанавливается.

---

## 2026-06-05 — Focus Session не стартовал из пустой формы

**Симптомы** — при клике «Начать фокус» без ввода Shell показывал `An object could not be cloned`; правая idle-панель визуально распадалась, а задача Delphi была отдельным select вместо Horologion-style `@` mention.
**Где жило** — `platform/desktop/src/components/FocusCommandPanel.vue::start`, `platform/desktop/electron/focus-session.ts::listTasks`, `platform/desktop/electron/focus-session.ts::startFocusSession`.
**Root cause** — renderer отправлял в Electron IPC Vue reactive/proxy array (`selectedBlocklistIds.value`) как `categoryIds`; structured clone не умеет клонировать Proxy, поэтому main-process handler даже не получал нормальный payload. UI дополнительно разделил цель и задачу на два независимых поля, хотя исходная модель Horologion выбирала task mention внутри единого текстового поля.
**Fix** — `FocusCommandPanel` теперь собирает IPC payload через `buildFocusSessionStartInput(...)`, который копирует reactive массивы в plain arrays перед `ipcRenderer.invoke`. Форма фокуса вернулась к Horologion-style модели: одно поле цели с `@` mention для незавершённых Delphi-задач, длительность выбирается через dropdown `25/45/60/90/Свое время`; правая панель `Сейчас` удалена полностью. Управление текущей сессией вынесено в отдельные Shell commands `kepler:focus-toggle`, `kepler:focus-pause`, `kepler:focus-resume`, `kepler:focus-skip`, `kepler:focus-complete`, как command-set модель Raycast Focus. `focus-session.ts::listTasks` фильтрует done/canceled/trashed задачи.
**Регрешн-защита** — `bun test tests/unit/focus-command-payload.test.ts`; `bun run shell:typecheck`; `bun run lint`; `bun run --cwd platform/desktop build:js:shell`; visual screenshots `.tmp/visual/2026-06-05-shell-command-surfaces-clipboard/{shell-focus-idle-720x460.png,shell-focus-720x460.png}`.
**Prevention** — Любой payload из Vue renderer в Electron IPC должен быть plain-data DTO. `ref([])` / `reactive([])` нельзя передавать в `ipcRenderer.invoke` напрямую; перед boundary делай `Array.from(...)` / object literal и добавляй structured-clone regression test для новых IPC DTO.

---

## 2026-06-05 — Встроенные команды Shell открывались вне Shell surface

**Симптомы** — при запуске `Начать фокус` или `Буфер обмена` Shell либо менял размер без смены контента, либо открывал отдельное окно/route. Clipboard history также терялась после рестарта процесса.
**Где жило** — `platform/desktop/electron/main.ts::showClipboardHistoryLauncher`, `platform/desktop/electron/clipboard-history.ts::openClipboardHistoryShell`, `platform/desktop/electron/commands.ts::kepler:focus-session`, `platform/desktop/electron/clipboard-history-store.ts::createClipboardHistoryStore`, `platform/desktop/src/views/LauncherView.vue`.
**Root cause** — в Shell одновременно существовали две competing surface модели: встроенный режим `LauncherView` для clipboard и fallback `BrowserWindow`/hash-host route для command views. Clipboard opener дополнительно менял bounds основного окна, а focus command уходил в Raycast-compatible host вместо Shell content slot. Store был in-memory-only, поэтому lifecycle процесса был ошибочно принят за lifecycle истории.
**Fix** — Clipboard History и Focus Session переведены в режимы `LauncherView`: opener'ы больше не создают fallback `BrowserWindow`, не меняют bounds shell'а и отправляют `kepler:clipboard-history:open-shell` / `kepler:focus-session:open-shell` в текущий launcher. `focus-session.ts` больше не импортирует Raycast host для built-in команды. Clipboard store получил instance-scoped JSON persistence, pruning по сроку/размеру и Settings tab с retention controls.
**Регрешн-защита** — `bun test tests/unit/clipboard-history-store.test.ts`; visual verify screenshots `.tmp/visual/2026-06-05-shell-command-surfaces-clipboard/{shell-clipboard-720x460.png,shell-focus-720x460.png,settings-clipboard-880x560.png}`; `bun run shell:typecheck`; `bun run lint`; `bun run --cwd platform/desktop build:js:shell`; `bun run docs:check`; `bun run ark:guard:writes`; `bun run ark:smoke`.
**Prevention** — Built-in Shell commands должны иметь ровно одного owner'а surface: `LauncherView` mode внутри текущего Shell. Opener built-in команды не должен иметь fallback на отдельный `BrowserWindow`/hash route, а persistence не должна зависеть от lifetime renderer/main process memory.

## 2026-06-05 — ArkClient не переподключался после delayed backend lock

**Симптомы** — при `bun run --cwd platform/desktop dev` shell логирует `[kepler-shell] kepler-backend not-installed: ArkClient unavailable`, затем backend через несколько секунд пишет `WS listening` и `lock-file written`, но renderer IPC `kepler:ark:request` продолжает падать с `ArkClient not ready (timeout)`.
**Где жило** — `platform/desktop/electron/main.ts::initArkClient`, `platform/desktop/electron/main.ts::awaitArkReady`, cold-start порядок `spawnBackend()` → `initArkClient()`.
**Root cause** — `initArkClient()` делал один handshake через `ensureKeplerRunning({ autoLaunch: false, waitMs: 10000 })`. Если dev backend писал lock позже этого окна, `initArkClient()` reject'ил ready promise и возвращался. Backend уже запускался, но shell не ставил новый retry на lock-ready/child-alive состояние, поэтому последующие `awaitArkReady()` создавали fresh promise, который никто больше не resolve'ил.
**Fix** — WIP: увеличено self-managed lock wait окно и добавлен bounded retry `initArkClient()` пока backend child жив, чтобы shell повторил handshake после delayed lock.
**Регрешн-защита** — TBD: нужно добавить тест/ручной dev-start proof на сценарий delayed backend lock.
**Prevention** — TBD после проверки.

---

## 2026-06-04 — focus-widget set-state стал extension-only

**Симптомы** — `window.kepler.focusWidget.setState(...)` из shell preload / launcher test helper начинает падать с `[kepler-shell] sender is not an extension` после добавления runtime permissions. Это ломает headless focus-widget tests и любые first-party shell windows, которые используют общий preload API.
**Где жило** — `platform/desktop/electron/focus-widget.ts::kepler:focus-widget:set-state` стал безусловно вызывать `assertExtensionSenderHostPermission`; `platform/desktop/electron/preload.ts` всё ещё exposes `focusWidget.setState` для shell renderer'ов, а `tests/e2e/helpers/horologion.ts::setFocusWidgetState` вызывает его из launcher window.
**Root cause** — permission fix смешал две разные sender-категории: untrusted extension renderers и first-party shell renderers. `webContentsToExtensionContext` знает только extension windows, поэтому shell window не может пройти extension permission lookup, хотя она не является user-installed кодом.
**Fix** — Добавлен `assertExtensionSenderHostPermissionIfExtension(...)`: если sender является extension window, `focus.control` enforce'ится как раньше; если sender — first-party shell renderer, handler допускает вызов. `focus-widget.ts` переведён на этот helper. Дополнительно e2e selector для кнопки «Ещё» переведён с устаревшего `.icon-btn` на role/name locator, потому что `@kosmos/visuals` IconButton больше не гарантирует этот CSS-класс.
**Регрешн-защита** — `bunx playwright test --config playwright.config.ts tests/e2e/focus-widget-controls.spec.ts` сначала воспроизвёл `[kepler-shell] sender is not an extension`, затем прошёл 7/7 после фикса. Unit permission tests также прошли.
**Prevention** — Main-process permission checks должны различать sender classes: user-installed extension renderer, trusted first-party extension renderer, shell renderer и internal widget renderer. Нельзя применять extension-only lookup к shared preload IPC только потому, что один из callers — extension.

---

## 2026-06-04 — ARK legacy/usage upserts использовали SQLite REPLACE

**Симптомы.** Часть ARK write helpers для legacy Delphi entities, usage entities, `sync_kv` и pending-object replay использовала `INSERT OR REPLACE`. Для таблиц с foreign keys / sync semantics это опасно: SQLite `REPLACE` удаляет старую строку и вставляет новую, а не обновляет её in-place.
**Где жило.** `core/ark/crates/ark-core/rust/src/db.rs::upsert_todo`, `upsert_project`, `upsert_area`, `upsert_tag`, `upsert_heading`, `insert_pending_object`, `upsert_tracked_app`, `upsert_usage_session`, `upsert_usage_event`, `set_sync_kv`; устаревший комментарий в `platform/runtime/src/focus.rs`.
**Root cause.** Старый Delphi/usage CRUD был написан до object-model regression tests against REPLACE semantics. Позднее `objects` / `object_types` / `object_links` уже перешли на update-in-place, но legacy/usage helpers остались на прежнем SQL и могли получить delete+insert side effects при повторном apply/upsert.
**Fix.** Все перечисленные helpers переведены на `INSERT ... ON CONFLICT(...) DO UPDATE SET ...`; внешний API и набор колонок не менялись. Комментарий backend focus обновлён на `upsert_object_type`, чтобы `rg "INSERT OR REPLACE"` не оставлял ложное ощущение разрешённого паттерна.
**Регрешн-защита.** `cargo test -p ark-core` — 166 unit tests, RPC tests, `proptest_invariants`, relay/sync integration. Дополнительно `rg -n "INSERT OR REPLACE|REPLACE INTO" core/ark/crates/ark-core/rust/src platform/runtime/src` больше не находит production SQL.
**Prevention.** Для syncable или FK-linked SQLite tables запрещён `REPLACE`: он является delete+insert и меняет lifecycle строки. Upsert helpers должны использовать `ON CONFLICT DO UPDATE`; если нужна destructive replacement, она должна называться явно и иметь отдельный regression test.

---

## 2026-06-04 — Dictation теряет речь после 30 секунд и autostart показывает ложную ошибку

**Симптомы.** Пользователи видят два эффекта: Settings → «Автозапуск с Windows» после включения показывает «Не удалось применить настройку», хотя автозапуск реально включился; диктовка длиннее ~30 секунд теряет часть речи после 30-й секунды и транскрибирует только основной/ранний фрагмент.
**Где жило.** `platform/desktop/src/views/SettingsView.vue:onToggleAutostart` делает немедленный `get()` после `set()` и сравнивает с desired; `platform/desktop/src/views/DictationPillView.vue:stopAndSubmit` отправляет один WAV; `platform/runtime/src/dictation/groq.rs::transcribe` отправляет этот WAV в Groq одним multipart-запросом.
**Root cause.** Autostart UI трактует мгновенный readback Electron/Windows autorun state как строгую postcondition, хотя Windows autorun запись уже могла быть применена, а `getLoginItemSettings()`/`launchItems` могут обновиться с задержкой или неполным match'ем. Dictation использует file-based Whisper как single-shot long-form transcription, хотя Groq/Whisper long-form аудио оптимально обрабатывается 30-секундными сегментами; без chunking модель может суммаризировать/обрезать хвост вместо дословного продолжения.
**Fix.** Settings UI больше не показывает «Не удалось применить настройку» из-за немедленного mismatch после успешного `autostart.set`: успешный IPC-write оптимистично выставляет desired state, а последующие открытия Settings синхронизируют реальное состояние через обычный `loadGeneral()`. `groq.rs::transcribe` теперь перед отправкой парсит renderer-generated 16-bit PCM WAV и режет long-form audio на ≤30s WAV chunks; каждый chunk отправляется в Groq отдельно, transcript parts склеиваются в исходном порядке.
**Регрешн-защита.** `tests/unit/settings-autostart-ui.test.ts` фиксирует, что успешный set не превращается в ложную ошибку UI. `platform/runtime/src/dictation/groq.rs::tests::{split_wav_for_transcription_chunks_long_audio_by_30_seconds,transcribe_posts_long_audio_in_ordered_chunks}` фиксируют 61s → 30/30/1 chunks и три Groq POST с конкатенацией transcript.
**Prevention.** Native OS settings readback не всегда является мгновенным commit proof; если write API не бросил exception, UI не должен показывать ошибку только из-за immediate eventual-consistency mismatch. Для Whisper/Groq long-form audio не отправляй пользовательскую запись одним запросом только потому что API принимает файл: если модельная рекомендация говорит про рабочее окно сегментов, segmentation должен быть частью backend transport layer, чтобы pending/retry сохраняли исходный файл, а provider получал стабильные chunks.

## 2026-06-04 — Akasha EPUB parser не ограничивал размер архива

**Симптомы.** Akasha принимала EPUB bytes без явных лимитов на размер исходного файла, количество ZIP entries, суммарный распакованный размер и размер отдельных entries. Специально подготовленный EPUB мог привести к чрезмерной памяти/CPU при распаковке или построении reader blocks.
**Где жило.** `incubator/akasha/src/lib/epub.ts` — `readEpubBytes()`, `readZipEntries()`, `readZipEntry()` и цикл построения `blocks`.
**Root cause.** Parser уже безопасно превращал XHTML в text nodes, но ZIP-level guardrails остались implicit: код доверял central directory metadata и начинал читать entries без budget checks.
**Fix.** Добавлены лимиты: 80 МБ на исходный EPUB, 4000 ZIP entries, 200 МБ суммарного uncompressed payload, 20 МБ на entry, 8 МБ на cover image и 60000 reader blocks. Oversized inputs fail fast до распаковки/рендера.
**Регрешн-защита.** `bun test tests/unit/akasha-epub-guardrails.test.ts`; `bunx playwright test tests/e2e/extensions-contract.spec.ts --grep akasha`; `bun run shell:typecheck`; `bun run shell:build`.
**Prevention.** Любой parser пользовательских архивов должен иметь явный budget на compressed input, central directory, uncompressed output и итоговую UI-модель. Без этого “безопасный XHTML renderer” всё ещё остаётся уязвимым к resource exhaustion.

## 2026-06-04 — Raw ARK writes через invokeOperation теряли device_id

**Симптомы.** Raw write-запросы через `ArkClient.invokeOperation()` могли записывать HLC в `lan_sync.version_vector` под `ark-core-rpc-local` или под переданным извне `device_id`, а не под стабильным device id текущего shell slot'а.
**Где жило.** `core/ark/packages/ark/src/ark-client.ts:783` прокидывал escape hatch request без нормализации local write identity; extension bridge в `platform/desktop/electron/extension-host.ts` использовал именно этот path для `window.kepler.ark.request(...)`.
**Root cause.** Typed SDK methods (`objects.upsert`, `objectTypes.upsert`, usage writes) вручную добавляли `device_id: this.opts.deviceId`, но public generic escape hatch остался прозрачным. После появления extension bridge этот escape hatch стал production write path'ом, поэтому sync identity зависела от raw params.
**Fix.** `ArkClient.invokeOperation()` теперь нормализует все local write operations (`upsert_*`, `delete_*`, `batch_upsert_todos`) через `withLocalWriteDeviceId()` и всегда ставит `device_id` из `ArkClientOptions.deviceId`, перекрывая spoofed raw value.
**Регрешн-защита.** `bun test tests/unit/ark-client-invoke-device-id.test.ts`; `bunx playwright test tests/e2e/extension-permissions.spec.ts` проверяет фактический `lan_sync.version_vector`: extension пытается передать `spoofed-extension-device`, а HLC заканчивается на `kepler-shell-test-extension-permissions-allow`.
**Prevention.** Любой generic RPC escape hatch обязан применять те же sync invariants, что typed SDK методы. Device identity — свойство локального клиента, а не доверенный параметр renderer/extension payload.

## 2026-06-04 — Delphi runtime притворялся multi-space приложением

**Симптомы.** Delphi extension всё ещё мог показывать/держать код legacy spaces и P2P sync: `SpaceSetup`, settings-вкладка «Пространства», fake `KEPLERDEFAULT`, local JSON/local DB fallback и `lan-sync:*` no-op'ы. Пользовательский runtime выглядел как старый standalone app, хотя source of truth уже single ARK DB.
**Где жило.** `products/delphi/src/App.vue` запускал `activateSpace()` и показывал `SpaceSetup`; `components/settings/SpacesSettingsTab.vue` держал UI управления пространствами; `lib/electron-api-shim.ts` возвращал fake space responses; `store/todos.ts` после изменений посылал legacy LAN broadcast/local DB side effects.
**Root cause.** Delphi был перенесён в extension через compatibility shim, но часть old standalone shell lifecycle осталась подключённой к runtime graph. Чтобы UI не падал, shim начал возвращать no-op/fake значения, и это замаскировало удалённую концепцию spaces вместо настоящего cleanup.
**Fix.** Electron bootstrap в `App.vue` теперь сразу грузит `ark:listDelphiTasks` из single ARK DB. `SpaceSetup`, `SpacesSettingsTab`, space-service, local JSON/local DB fallback и legacy LAN protocol удалены. Sidebar/settings больше не показывают «Пространства». Store сохраняет задачи только через ARK task bridge, без legacy LAN broadcast/local DB writes. Shim оставлен минимальным: task CRUD/time-entry reads; неизвестные old channels получают `warnOnce()` + `null`, без fake spaces.
**Регрешн-защита.** `bun run shell:typecheck`; `bunx playwright test tests/e2e/delphi-legacy-cleanup.spec.ts`; visual screenshot `.tmp/visual/2026-06-04-delphi-legacy-cleanup/delphi-main-no-space-setup.png`.
**Prevention.** Compat shim допустим только как переходник для реально используемых call-site'ов. Удалённая продуктовая концепция не должна жить как fake UI/no-op API: если runtime больше single-source-of-truth, dead screens and fake defaults нужно убирать из import graph.

## 2026-06-04 — Extension permissions не ограничивали ARK RPC

**Симптомы.** Любое установленное Vue extension-окно могло вызвать `window.kepler.ark.request(operation, params)` с произвольной backend/ARK operation, даже если manifest декларировал permissions или не декларировал их вовсе.
**Где жило.** `platform/desktop/electron/extension-host.ts:104` описывал `permissions` как documentation-only; `platform/desktop/electron/extension-host.ts:1288` прокидывал `kepler:extension:ark:request` в общий `arkRequest` без проверки sender extension capabilities.
**Root cause.** Permission model остановился на install-dialog/documentation layer, а runtime bridge был спроектирован как trusted first-party proxy: `webContentsToExtensionId` определял владельца окна, но результат не использовался для authorization. После появления `.kext` install/user override flow это стало security boundary bug'ом: untrusted user-installed code получал тот же operation surface, что bundled apps.
**Fix.** Runtime capability model вынесен в `platform/desktop/electron/extension-permissions.ts` и подключён в `extension-host.ts`: ARK requests, ARK event subscriptions и userData IPC теперь проверяют manifest permissions для user-installed extension'ов. `focus-widget:set-state` защищён `focus.control`. First-party trust определяется source (`dev` / `bundled`), а не id, поэтому user-installed override с id `eden` не наследует broad trust. IPC reject'ит `params.operation`, чтобы checked operation нельзя было перезаписать при merge.
**Регрешн-защита.** `bun test tests/unit/extension-permissions.test.ts`; `bunx playwright test tests/e2e/extension-permissions.spec.ts`; `bunx playwright test tests/e2e/extensions-contract.spec.ts`; `bunx playwright test tests/e2e/commands-architecture.spec.ts`; `bun run shell:typecheck`; `bun run shell:build`.
**Prevention.** Любое поле manifest'а, которое выглядит как security boundary (`permissions`, `apiVersion`, source/origin), должно иметь main-process enforcement до появления install/update flow. Trust нельзя выдавать по extension id: user-installed override может называться как first-party app. IPC proxy должен проверять ровно тот operation payload, который будет forwarded, и запрещать shadow fields вроде `params.operation`.

## 2026-06-04 — ARK upsert удаляет object_links через SQLite REPLACE

**Симптомы.** Повторный upsert существующего объекта или типа мог незаметно удалить связи графа: обновление заметки/задачи/тега потенциально сносило `object_links`, которые на них ссылались.
**Где жило.** `core/ark/crates/ark-core/rust/src/db.rs:609` (`upsert_object_type`), `core/ark/crates/ark-core/rust/src/db.rs:875` (`upsert_object`), `core/ark/crates/ark-core/rust/src/db.rs:1389` (`upsert_object_link`).
**Root cause.** В SQLite `INSERT OR REPLACE` реализован как delete старой строки плюс insert новой. Для `objects` и `object_types` это проходило через FK `ON DELETE CASCADE`, поэтому обычный upsert родительской строки мог каскадно удалить дочерние `object_links`/`objects`.
**Fix.** Generic object model helpers переведены на `INSERT ... ON CONFLICT(id) DO UPDATE SET ...`: `upsert_object_type`, `upsert_object` и `upsert_object_link` теперь обновляют существующую строку без delete+insert side effects.
**Регрешн-защита.** `cargo test --manifest-path crates\ark-core\rust\Cargo.toml --lib upsert_object` покрывает сохранение входящих/исходящих `object_links` при upsert объекта, сохранение objects/links при upsert object type и in-place update существующего link row.
**Prevention.** В SQLite `REPLACE` нельзя использовать как синоним upsert для таблиц с FK-зависимостями или потенциальными зависимыми строками. Для canonical ARK CRUD helper'ов upsert должен быть явным `ON CONFLICT DO UPDATE`, а регрессия должна проверять сохранение зависимых строк, не только итоговые поля родительской записи.

## 2026-06-01 — Dashboard usage показывает битые app icons

**Симптомы.** В Dashboard → «Затреканное время» часть строк показывала browser broken-image placeholder вместо иконки приложения.
**Где жило.** `platform/desktop/src/dashboard/store.ts` выбирал `RawTopAppEntry.iconRef` перед inline `app_index.list_all.icon_path`; `platform/desktop/src/dashboard/UsageTable.vue` рендерил `<img>` без fallback on-error.
**Root cause.** Usage analytics может вернуть stale/renderer-недоступный `tracked_apps.icon_ref` из usage-domain данных. При этом `app_index.list_all` уже специально инлайнит иконки как `data:image/png;base64,...`, но Dashboard отдавал приоритет `iconRef`, поэтому renderer пытался загрузить непригодный путь и показывал native broken-image icon.
**Fix.** Dashboard теперь выбирает иконку через `chooseUsageIconRef()`: сначала renderer-safe inline icon из `app_index.list_all`, затем только inline `data:image/*` из usage-domain; file/path-like `iconRef` без app-index fallback отбрасывается. `UsageTable` дополнительно скрывает `<img>` на `error`, чтобы единичная битая data-url не показывала browser placeholder.
**Регрешн-защита.** `bun platform/desktop/src/dashboard/store.regression.mjs`, `bun run --cwd platform/desktop typecheck`.
**Prevention.** Renderer-facing image fields должны быть либо уже inline/data URL, либо проходить через явный sanitizer/fallback. Нельзя смешивать storage/internal icon refs и browser-safe image src в одном приоритете: если endpoint специально инлайнит assets для renderer'а, UI должен предпочитать именно его.

## 2026-06-01 — Usage tracker спамит process window check failed

**Симптомы.** В логах `kepler-backend` каждую секунду повторялись строки `[usage-tracker] process window check failed for pid ...: Присоединенное к системе устройство не работает. (0x8007001F)` для одних и тех же PID.
**Где жило.** `platform/runtime/src/usage_tracker/mod.rs` в loop проверки `process_window_state`; Win32 probe живёт в `platform/runtime/src/usage_tracker/windows_capture.rs`.
**Root cause.** Tracker после ошибки Win32 probe классифицировал процесс как `AliveHidden`. Для умершего или transient-недоступного PID это оставляло `ActiveSession` в карте навсегда: runtime уже не рос, но каждый следующий tick снова вызывал тот же probe и снова писал ошибку в stderr.
**Fix.** Ошибка `process_window_state` теперь проходит через `process_window_state_or_dead`: если tracker не может подтвердить, что PID всё ещё принадлежит ожидаемому exe path, активная session считается `Dead`, финализируется и удаляется из `active_sessions`.
**Регрешн-защита.** `cargo test --manifest-path services\kepler-backend\Cargo.toml usage_tracker --lib` покрывает `process_window_probe_error_ends_session`: probe `Err("...0x8007001F")` должен возвращать `ProcessWindowState::Dead`.
**Prevention.** Для process accounting “не могу проверить identity процесса” нельзя трактовать как “процесс жив, но hidden”. Hidden допустим только после успешного чтения exe path и проверки совпадения identity; ошибки identity probe должны завершать session или иметь явный bounded retry.

## 2026-06-01 — Usage tracker занижает playtime игр

**Симптомы.** Dashboard показывал около 2 часов The Witcher 3, хотя пользователь реально играл примерно 10 часов. Read-only проверка `%APPDATA%\Kosmos\ark.db` подтвердила: недостающего времени нет ни в live DB, ни в WAL, ни в backup'ах.
**Где жило.** `platform/runtime/src/usage_tracker/mod.rs` держал один foreground-session state и завершал session при уходе с foreground окна; `platform/runtime/src/usage_tracker/windows_capture.rs` сэмплил только `GetForegroundWindow`.
**Root cause.** Usage tracker смешал две разные метрики: foreground activity и playtime/runtime процесса. Для игр пользователь ожидает «пока процесс игры жив», а код считал только «пока окно игры foreground». Дополнительно tracker task мог завершиться навсегда после одной Win32/DB ошибки, потому что `spawn()` логировал `FATAL`, но caller выбрасывал `JoinHandle`.
**Fix.** `usage_sessions` получил отдельный `runtime_ms` с additive migration/backfill из старого `foreground_ms + idle_ms`. Tracker теперь держит карту живых process sessions по `(tracked_app_id, pid)`, считает `runtime_ms` каждую секунду до завершения процесса, сохраняет session каждый tick и не завершает task из-за единичной capture/DB ошибки. Dashboard и game playtime aggregates используют `runtime_ms`, а `foreground_ms` оставлен отдельной диагностической метрикой.
**Регрешн-защита.** `cargo test --manifest-path crates\ark-core\rust\Cargo.toml usage_analytics_snapshot_includes_summary_and_zero_filled_trend`, `cargo test --manifest-path crates\ark-core\rust\Cargo.toml usage_game_playtime_summary_matches_bindings_and_range`, `cargo test --manifest-path crates\ark-core\rust\Cargo.toml test_init_schema_adds_usage_runtime_ms_to_existing_sessions`, `cargo test --manifest-path services\kepler-backend\Cargo.toml usage_tracker --lib`.
**Prevention.** В usage-домене нельзя использовать foreground как proxy для playtime. Для игр и долгоживущих приложений основная метрика — process runtime, foreground/idle — только дополнительные срезы. Любой бесконечный tracker loop должен переживать transient capture/persistence ошибки и иметь тест на отличие runtime от foreground.

## 2026-06-01 — Dashboard не показывает usage tracker

**Симптомы.** В Kepler Dashboard при открытии базы видны почти все пользовательские данные, но нет записей затреканных приложений / тайм-трекинга, хотя usage tracker пишет их в ARK DB.
**Где жило.** `platform/desktop/src/dashboard/store.ts` грузил только `list_object_types`, `list_objects` и `list_objects_by_type`; `platform/desktop/src/views/DashboardView.vue` строил sidebar только из object types.
**Root cause.** Dashboard был реализован как object browser поверх универсальной ARK object model. Usage tracker хранит данные в отдельных таблицах `tracked_apps`, `usage_sessions`, `usage_events`, которые не представлены в `object_types`, поэтому корректно записанные usage rows не могли появиться ни в sidebar, ни в таблице. При этом read-only aggregate endpoint `get_usage_analytics` уже существовал, но Dashboard его не использовал.
**Fix.** Dashboard получил отдельный sidebar-пункт «Затреканное время», который загружает `get_usage_analytics` и показывает aggregate по приложениям: название процесса, суммарное foreground-время, display name, число сессий, idle-время, последний запуск и normalized path. Object browser остался без raw SQLite и продолжает использовать ARK read-only IPC.
**Регрешн-защита.** `bun run --cwd platform/desktop typecheck`, `bun run format:check platform/desktop/src/dashboard/types.ts platform/desktop/src/dashboard/store.ts platform/desktop/src/dashboard/UsageTable.vue platform/desktop/src/views/DashboardView.vue docs-site/agents/postmortems.md`, `bun run docs:check`.
**Prevention.** Dashboard нельзя считать «полным просмотром базы», если он перечисляет только `object_types`. Для ARK-инспектора каждый non-object domain (`tracked_apps` / `usage_sessions` / `usage_events`, sync metadata, future side tables) должен иметь явный navigation surface или осознанно задокументированное исключение; иначе данные будут записываться корректно, но оставаться невидимыми в UI.

## 2026-05-30 — Focus widget pause ignores operation response

**Симптомы.** В focus widget кнопка «Пауза» не выглядела рабочей: клик уходил в backend, но сам виджет продолжал показывать running-состояние/кнопку «Пауза» и автономный тик мог продолжаться до следующего внешнего push.

**Где жило.** `platform/desktop/electron/focus-widget.ts:402` — `invokePomodoro()` вызывал `pomodoro.pause` / `pomodoro.resume`, но игнорировал returned state и полагался только на backend events.

**Root cause.** `PomodoroHost::pause()` и `PomodoroHost::resume()` в `platform/runtime/src/pomodoro_host.rs` intentionally не эмитят `pomodoro_phase_changed`: фаза не меняется, операция только меняет `isPaused`, `isRunning`, `remainingMs` и `phaseEndsAtMs`, возвращая новый snapshot в RPC response. Focus widget main-process path ожидал event-driven обновление для всех pomodoro ops, поэтому для pause/resume локальный `currentState` оставался stale.

**Fix.** `invokePomodoro()` теперь типизированно читает snapshot из `client.invokeOperation<PomodoroEventState>()` и сразу применяет `setFocusState(deriveFocusStateFromBackend(state))`. Event subscription остаётся для tick/phase_changed/finished, но inline controls больше не зависят от события там, где backend contract возвращает state response без event.

**Регрешн-защита.** `tests/e2e/focus-widget-controls.spec.ts` расширил pause test: после клика «Пауза» он проверяет появление кнопки «Продолжить», `focusWidget.getState().isPaused === true` и `phaseEndsAtMs === null`; затем кликает «Продолжить» и проверяет возврат к running state. Перед fix этот тест падал на отсутствии `.btn[aria-label="Продолжить"]`. После fix прошли `bun run --cwd platform/desktop typecheck`, `bun run format:check platform/desktop/electron/focus-widget.ts tests/e2e/focus-widget-controls.spec.ts docs-site/agents/postmortems.md`, `bun run test:e2e tests/e2e/focus-widget-controls.spec.ts`, `bun run --cwd platform/desktop build:js`.

**Prevention.** Для IPC/RPC controls нельзя предполагать, что mutation всегда придёт вторым событием. Если backend operation возвращает authoritative snapshot, UI-owner должен применить response немедленно; event stream — это синхронизация для внешних изменений и тиков, а не единственный способ обновить state после user action.

## 2026-05-30 — Focus block dev bundle loses dynamic imports

**Симптомы.** При `bun run --cwd platform/desktop dev` после старта Kepler в логах появлялись `[focus-block] service path failed: pingService is not a function` и `[extension-host] focus block apply failed: TypeError: isFocusServiceAutoInstallDeclined is not a function`. Focus-block fallback мог падать до helper/service path, хотя приложение продолжало запускаться.

**Где жило.** `platform/desktop/electron/focus-block.ts:169` и `platform/desktop/electron/focus-block.ts:218` — dynamic imports `./focus-service` / `./settings-window`; `platform/desktop/electron/main.ts` — Electron main entry экспортировал только `awaitArkReady` / `shouldShowLauncherOnStartup`.

**Root cause.** Vite/Rolldown code-splitting вынес `focus-block.ts` в отдельный dynamic chunk и переписал его dynamic imports модулей, уже попавших в main entry, в `import("./main.js")`. Но entry bundle не реэкспортировал нужные функции из `focus-service.ts` и `settings-window.ts`; на TypeScript уровне всё было валидно, а runtime module namespace в dev/prod bundle не содержал `pingService` / `isFocusServiceAutoInstallDeclined`.

**Fix.** `platform/desktop/electron/main.ts` теперь явно реэкспортит `getServiceStatus`, `runServiceCliElevated`, `pingService`, `sendViaPipe`, `isFocusServiceAutoInstallDeclined` и `setFocusServiceAutoInstallDeclined`. Это сохраняет текущую chunking-схему, но делает runtime namespace `main.js` совместимым с тем, во что bundler превращает dynamic imports из `focus-block.ts`.

**Регрешн-защита.** `bun run --cwd platform/desktop typecheck`, `bun run --cwd platform/desktop build:js`, `bun run format:check platform/desktop/electron/main.ts docs-site/agents/postmortems.md`, `bun run docs:check` прошли после fix. Дополнительно проверен собранный `platform/desktop/dist-electron/main.js`: export namespace содержит `pingService`, `sendViaPipe`, `isFocusServiceAutoInstallDeclined` и остальные API, которые импортирует `focus-block-*.js`.

**Prevention.** Если Electron main использует dynamic import для модулей, которые уже импортированы entrypoint'ом, обязательно проверять не только TypeScript, но и форму production/dev bundle. Code-splitting может переписать source-level imports на entry module namespace; тогда нужные функции должны быть entry exports либо модуль должен быть вынесен в отдельный shared chunk явно.

## 2026-05-28 — Focus widget label flicker and noisy controls

**Симптомы.** Pomodoro focus widget визуально дёргался при задаче: label мог прыгать между конкретным названием и дефолтным pomodoro label. Сам виджет был перегружен: controls постоянно занимали место вместо базового режима «время + задача».

**Где жило.** `platform/desktop/electron/focus-widget.ts` — `setFocusState` без различения качества источников сливал patches от backend events и renderer push. `platform/desktop/src/views/FocusWidgetView.vue` — controls были всегда видимыми при active session.

**Root cause.** Focus widget имеет два источника правды: backend events дают надёжный lifecycle/tick, а Horologion renderer знает более богатый UI context (draft title, tasks, focus blocking). Main process принимал последний patch целиком, поэтому более бедный backend tick мог перезаписать уже показанный конкретный label дефолтом. В UI controls были частью обычного flex layout, поэтому даже idle visual state выглядел как toolbar, а не compact status widget. Первый hover-only fix повесил reveal на область с `-webkit-app-region: drag`; в реальном Electron drag-region не ведёт себя как обычная DOM hover target, поэтому кнопки пропали и не раскрывались.

**Fix.** `setFocusState` теперь сохраняет уже известный конкретный label, когда следующий active patch в том же mode несёт только дефолтный `Фокус`/`Перерыв`. `FocusWidgetView` переведён в compact baseline: время и label занимают обычную no-drag hover area, а `IconButton` controls из `@kosmos/visuals` появляются на их месте поверх content overlay. Перетаскивание вынесено в отдельную `.drag-handle` с `GripVertical`, окно стало уже, левая accent-полоса удалена, а mode-индикация стала заполняющим progress-fill внутри плашки через новый `totalSec` в focus widget state.

**Регрешн-защита.** `tests/e2e/focus-widget-controls.spec.ts` добавил regression `backend generic label не перетирает конкретное название` и проверяет, что controls скрыты до hover/focus, content-zone `no-drag`, handle-zone `drag`, а pause/skip/stop остаются кликабельны. `bun run --cwd platform/desktop typecheck`, `bun run format:check ...`, `bun run --cwd platform/desktop build:js`, `bun run test:e2e tests/e2e/focus-widget-controls.spec.ts` прошли после fix.

**Prevention.** Для state, который собирается из нескольких источников разной полноты, merge обязан учитывать качество поля, а не только “последний patch победил”. Backend lifecycle/tick может быть authoritative по времени, но не по UI context; renderer-owned label/task context нельзя затирать дефолтами из более бедного события. В Electron floating widgets нельзя совмещать hover-only controls с `-webkit-app-region: drag` на той же DOM-зоне; drag должен жить в отдельной маленькой области, а hover/click controls — в `no-drag`.

## 2026-05-27 — Akasha EPUB blocks overlap on resize and miss source semantics

**Симптомы.** При сужении/расширении окна Akasha расстояния между абзацами не пересчитывались, текст начинал налезать друг на друга, а часть EPUB-стилей из реальной книги выглядела как обычный плоский текст. Копирование выделения было неочевидным.

**Где жило.** Старый GPUI reader Akasha — virtual-list item heights оценивались по фиксированной ширине `70ch` и не зависели от viewport width. Parser учитывал только базовые `strong/em`, но не EPUB semantic spans вроде `epub:type="bridgehead"` и inline tags вроде `cite`.

**Root cause.** Виртуализация получила стабильные item sizes, но эти sizes описывали “идеальную” ширину, а не текущую ширину reader column; при wrap на узком окне GPUI рисовал больше строк, чем virtual list зарезервировал места. Семантические EPUB-теги терялись до UI-слоя, поэтому Apple Books tokens не могли примениться к ним визуально.

**Fix.** Akasha теперь хранит текущую ширину reader column и пересчитывает `reader_item_sizes` при изменении `window.viewport_size()`, поэтому virtual-list rows растут вместе с переносами текста. Paragraph/list blocks получили Apple Books-style vertical rhythm через em-based bottom margins. Parser теперь сохраняет `epub:type="bridgehead"` как bold и `cite` как emphasis. После drag-selection появляется маленькая floating-подсказка с `Copy`, которая вызывает стандартный `TextView` copy action.

**Регрешн-защита.** `cargo test -p akasha` добавил regression tests на responsive item-height (`estimated_block_height_tracks_reader_width`), paragraph gap (`paragraph_estimate_includes_apple_books_gap`) и EPUB semantic inline tags (`preserves_epub_bridgehead_and_cite_semantics`). `cargo check -p akasha`, `bun run --cwd platform/desktop typecheck`, `bun run ark:guard:writes` прошли после fix.

**Prevention.** Для virtualized reader/feed UI item estimate должен принимать тот же layout width, при котором реально рисуется текст; фиксированные `ch`-оценки допустимы только для фиксированной колонки. EPUB parser обязан сохранять не только HTML-теги, но и распространённые semantic attributes (`epub:type`) до UI-слоя, иначе typography tokens применяются к уже обеднённой модели.

## 2026-05-27 — Akasha reader без scrollbar, незаметное открытие книги и медленный native dev launch

**Симптомы.** В Akasha не было видимого scrollbar у reader'а, повторное открытие другой книги было неочевидным, а запуск native Akasha из Kepler в dev ощущался медленным.

**Где жило.** Старый GPUI reader Akasha — `v_virtual_list` использовался без отдельного `Scrollbar::vertical`, а кнопка открытия была вторичной в toolbar. `incubator/akasha/manifest.json` и `platform/desktop/scripts/build-extensions.mjs` — dev native path/build указывали на debug binary.

**Root cause.** `gpui_component::v_virtual_list` виртуализует scroll surface, но не рисует scrollbar автоматически. Для native extension dev flow был выбран `target/debug/akasha.exe`, что удобно для отладки, но плохо совпадает с ожиданием “открывается как приложение”.

**Fix.** Reader virtual-list теперь обёрнут в `relative` container с `Scrollbar::vertical(&reader_scrollbar)`. Кнопка открытия книги вынесена первой в toolbar и переименована в «Открыть книгу». Native dev executable переключён на `target/release/akasha.exe`, а `build:extensions` собирает native extensions через `cargo build --release -p <pkg>`.

**Регрешн-защита.** `cargo check -p akasha` проверяет scrollbar/layout интеграцию; `cargo test -p akasha` проверяет reader state/parser; `bun run --cwd platform/desktop typecheck` проверяет manifest/native host types.

**Prevention.** Для virtual list в GPUI Components scrollbar не появляется сам — всегда добавлять `Scrollbar::vertical` явно. Для native extensions dev path должен совпадать с пользовательским ожиданием latency; debug binary лучше оставлять для ручного debugging, а launcher должен запускать release build.

## 2026-05-27 — Akasha continuous reader грузит CPU и лагает на скролле

**Симптомы.** После перехода на непрерывный reader скролл в Akasha стал заметно лагать, а приложение грузило ПК сильнее, чем ожидается от GPUI/Zed-like UI.

**Где жило.** Старый GPUI reader Akasha — reader собирал `Vec<ReaderBlock>` на каждом render и создавал GPUI/`StyledText` element для каждого блока книги сразу.

**Root cause.** Continuous flow был реализован визуально, но не архитектурно: вместо virtualized reader surface UI рендерил весь EPUB spine целиком. Для больших книг это означает сотни/тысячи text layout объектов на каждый render path, что ломает ожидаемую плавность GPUI.

**Fix.** Reader теперь кэширует flatten EPUB-блоков при открытии книги и пересчитывает только rough item sizes при изменении font size. Полный `.children(all_blocks)` заменён на `gpui_component::v_virtual_list`, который материализует только видимый `Range<usize>`; chapter navigation использует `VirtualListScrollHandle::scroll_to_item`.

**Регрешн-защита.** `cargo check -p akasha` проверяет интеграцию `v_virtual_list`; `cargo test -p akasha` оставляет зелёными parser/state regression tests. Proof loop: `.agent/tasks/2026-05-27-akasha-virtual-scroll/`.

**Prevention.** Для любых reader/feed/table UI “continuous” должен означать continuous user model, а не full materialization. Если список может быть длиннее нескольких экранов, сначала выбирать `uniform_list`/`v_virtual_list`, а уже потом навешивать rich text styling.

## 2026-05-27 — Akasha EPUB отображается как dump выбранной главы

**Симптомы.** Akasha показывала постоянную левую колонку глав и текст только выбранной главы справа. Список глав не скроллился, прокрутка текста не переходила бесшовно в следующую главу, заголовки/жирный/курсив EPUB терялись и весь текст выглядел плоским.

**Где жило.** Старый GPUI reader Akasha — парсер схлопывал XHTML в `Vec<String>` без типа блока и inline-стилей, а UI был построен вокруг `chapter_index` и рендера одной выбранной главы.

**Root cause.** MVP смоделировал EPUB как “chapter selector + selected chapter body”, а не как непрерывный читательский документ. Семантика HTML удалялась до UI, поэтому GPUI-рендерер уже не мог отличить заголовок от абзаца или жирный span от обычного текста.

**Fix.** Akasha parser теперь хранит структурные `ReaderBlock` + `InlineSpan` вместо плоских строк: heading level, paragraph/list/blockquote и bold/italic marks доходят до UI. Reader UI убрал постоянный sidebar, рендерит весь spine одним continuous scroll surface, а главы показывает через верхнюю scrollable panel; выбор главы скроллит основной reader к первому блоку этой главы.

**Регрешн-защита.** `cargo test -p akasha` покрывает fixture EPUB со spine-order блоками и отдельный кейс `preserves_reader_structure_and_inline_emphasis`, который перед fix'ом падал на потере `<h2>`, `<strong>` и `<em>`.

**Prevention.** Для reader'ов нельзя схлопывать входной формат в `String` до UI-слоя. Даже MVP должен передавать минимальную document model: block kind + inline marks + spine order, иначе любая следующая фича чтения превращается в reverse engineering уже потерянной HTML-семантики.

## 2026-05-26 — Kosmos System Service uninstall оставляет legacy service

**Симптомы.** После миграции `KeplerFocusSvc` → `KosmosSystemSvc` Settings мог показывать системный сервис как установленный даже после успешного uninstall.

**Где жило.** `platform/native-services/kepler-focus-svc/src/cli.rs:159` — `uninstall()` открывал один “первый найденный” сервис через `open_installed_service`.

**Root cause.** Compatibility helper был корректен для `status/start/stop`, где нужен один active service, но был переиспользован для `uninstall`, где семантика другая: upgrade-машина может легально иметь оба сервиса одновременно. Приоритет new-first приводил к удалению только `KosmosSystemSvc`; legacy `KeplerFocusSvc` оставался в SCM и следующий `status()` снова видел installed=true.

**Fix.** `uninstall()` больше не использует “первый найденный” service helper. Он проходит по `[KosmosSystemSvc, KeplerFocusSvc]`, для каждого найденного сервиса делает best-effort stop и delete, а отсутствие одного из имён считает idempotent success.

**Регрешн-защита.** `cargo test -p kepler-focus-svc uninstall_targets_new_and_legacy_service_names` проверяет, что uninstall-план всегда включает новое и legacy имя сервиса.

**Prevention.** Compatibility fallback и cleanup — разные операции. Fallback обычно должен выбирать один active target, а cleanup/migration должен рассматривать все legacy targets как независимые хвосты, которые могут одновременно существовать после upgrade.

## 2026-05-23 — Kepler: singleton conflict из-за pid reuse

**Симптомы.** После некорректного завершения kepler-backend (panic, kill, BSOD) при следующем запуске `bun run --cwd platform/desktop dev` backend бесконечно падает на старте с `FATAL setup: singleton conflict via lock-file`. Supervisor уходит в respawn-loop (1s → 5s → 30s → 60s → 120s), shell показывает `ArkClient not ready (timeout)`, IPC `kepler:ark:request` валится. Помогает только ручное удаление `%APPDATA%\Kosmos\kepler.lock.json`.

**Где жило.** `platform/runtime/src/main.rs:182-189` — гейт перед `SingletonGuard::acquire`. Использовал `platform/runtime/src/lock_file.rs:204` (`read_if_alive`), который через `platform/runtime/src/lock_file.rs:223` (`is_pid_alive` на Win — `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, ...)`) проверял только «жив ли PID», без валидации, что это именно kepler-backend.

**Root cause.** Pid-based liveness check — фундаментально ненадёжный механизм определения «работает ли мой сервис». Windows переиспользует освободившиеся PID'ы, и наблюдаемый эпизод тому пример: PID 14852 был записан в lock-файл предыдущим kepler-backend'ом в 17:56 (упал без cleanup'а), к 23:05 Windows отдала этот же PID запускающемуся electron-shell'у. Гейт честно отвечал «PID 14852 жив» (electron действительно жив), и backend отказывался стартовать. Хуже того: `is_pid_alive` принципиально не может различить «мой мёртвый процесс / переиспользованный PID живой electron'а / случайный chrome / explorer».

Архитектурно это была защита-в-глубину к настоящему механизму singleton'а — `SingletonGuard` на `kepler-singleton.lock.db` (SQLite WAL `BEGIN IMMEDIATE`). Этот lock работает корректно: kernel держит file handle, при любой смерти процесса handle освобождается, pid reuse физически не может его сломать. Но pid-гейт стоял **до** настоящего lock'а и отказывал раньше, чем тот успевал дать авторитетный ответ. То есть «защита в глубину» оказалась strict superset: первый слой ловил то, что второй пропустил бы корректно. JSON-файл должен быть discovery-метаданными для shell (ws_port, auth_token), а не gate'ом — параллель с Postgres'овым `postmaster.pid` (метаданные) vs. `flock` на data directory (настоящий gate).

**Fix.** Pid-based гейт убран. Новый pub helper `kepler_backend::singleton::acquire_clearing_stale_lock(lock_path, singleton_path)` делает `SingletonGuard::acquire` (SQLite WAL `BEGIN IMMEDIATE` — OS-level file lock), затем безусловно удаляет stale `kepler.lock.json` если он был. JSON остаётся discovery-метаданными, перезаписывается `lock_file::write_atomic` после `ws.bind`. Удаление stale JSON **сразу после acquire** (а не лениво при write_atomic) закрывает race-окно «старый ws_port на диске пока новый WS ещё не bind'нулся» — shell, прочитавший JSON в это окно, получит `ENOENT` → retry в supervisor'е, а не connect к мёртвому endpoint'у с истёкшим auth token'ом. Мёртвый код `lock_file::read_if_alive` и `lock_file::is_pid_alive` (вместе с Win-веткой через `OpenProcess` и Unix-веткой через `kill(pid, 0)`) удалён — иначе через полгода кто-то «починит» обратно, не разобравшись.

**Регрешн-защита.** `platform/runtime/src/singleton.rs::tests`:

- `stale_lock_with_live_unrelated_pid_does_not_block_acquire` — пишет `kepler.lock.json` с `pid = std::process::id()` (это cargo test binary — гарантированно живой, гарантированно НЕ kepler-backend) и проверяет, что `acquire_clearing_stale_lock` возвращает `Ok` + reported_pid + удалил файл. Тест прямо симулирует наблюдаемый сценарий pid reuse.
- `acquire_clearing_stale_lock_handles_missing_json` — first-time startup, JSON отсутствует.
- `acquire_clearing_stale_lock_removes_corrupt_json` — частично записанный JSON от прерванного `write_atomic` тоже не блокирует.
- `second_acquire_clearing_stale_lock_fails_with_already_running` — параллельный second-acquire падает с `SingletonError::AlreadyRunning`, и сообщение об ошибке содержит «Kepler» (часть контракта — diagnosability в логах supervisor'а).

Manual repro для AC7: запустить `bun run --cwd platform/desktop dev`, дождаться bind, `Stop-Process` на kepler-backend.exe без cleanup'а, дождаться respawn — должен подняться.

**Prevention.**

- **PID — это не identity процесса.** Любой код, говорящий «PID жив → процесс X жив», содержит скрытое допущение «PID не переиспользован». Допущение ломается всегда — Windows крутит счётчик быстро, Linux обнуляет на 32767 по умолчанию. Если нужно «работает ли мой сервис», ответ один: **OS-level lock на файле/сокете/named pipe**, который kernel освобождает на смерть процесса.
- **Discovery-метаданные ≠ gate.** Postgres: `postmaster.pid` содержит port + socket dir, но startup gate — это `flock` на data directory. SQLite: `<db>-wal` хранит WAL state, lock — POSIX advisory range lock на самой DB. Любой файл, который один процесс пишет на старте а другие читают для discovery, **не может одновременно быть и gate'ом** — потому что читатель не может атомарно «прочитать + захватить». Если в коде появляется паттерн «прочитать какой-то файл, проверить какой-то признак, решить можно ли стартовать» — это red flag.
- **Defense-in-depth не освобождает от корректности первого слоя.** Здесь pid-гейт стоял **до** настоящего `SingletonGuard` и отказывал раньше. В результате «защита» оказалась strict liability — первый слой ловил ровно те случаи, которые второй обработал бы корректно. Прежде чем добавлять «дополнительную проверку», спросить: что именно она ловит, чего не ловит настоящий механизм, и какой её false-positive rate? Если правильный слой работает — лишний только увеличивает поверхность отказа.
- **Lock-файл должен исчезать после graceful shutdown.** Уже есть (`main.rs:143` — `remove_file(&lock_path)` в shutdown path), но ungrateful shutdown оставит JSON на диске; новый код к этому resilient (удаляет stale при следующем старте). Если в проекте появятся другие discovery-файлы — same rule.

**Связанные правила.** [forbidden.md § Rust](/agents/forbidden) — Mutex poison recovery + `RUST_BACKTRACE=1` + `db_backup`. Этот случай — пример того, что singleton-механизмы должны быть на kernel-уровне; pid-based — анти-паттерн.

---

## 2026-05-23 — Kepler: file search показывает только файлы из %APPDATA% (ИСПРАВЛЕНО 2026-05-24)

**Симптомы.** File search в launcher находит только файлы под `%APPDATA%\Roaming\...`, не находит проекты на `C:\` / `D:\`.

**Где жило.** Гипотеза по `platform/runtime/src/file_index/scanner/ntfs.rs:13-29` (`scan_drive_root`) → service pipe fail → `Volume::new(\\.\C:)` fail (admin required) → `platform/runtime/src/file_index/scanner.rs:70` (`scan_walk_root`) fallback. Default roots в `scanner::default_roots()` **корректны** — `GetLogicalDrives` + `GetDriveTypeW`, lock_dir НЕ передаётся как root.

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

**Финальная проверка.** `cargo test -p kepler-backend --lib file_index::` → 25/25, `bun run --cwd platform/desktop typecheck` → clean, `bun run ark:guard:writes` → PASS.

**Prevention для будущего.** (1) Watcher state и settings state — независимые подсистемы; на стыке всегда фильтрация по current snapshot. (2) Платформо-зависимые понятия (hidden, executable) — никогда не сводить к POSIX convention. (3) Path-as-key — всегда canonicalize в одной точке (на write), не «store as-given, compare with lower()». (4) Validation должна повторять production path в миниатюре, не только первый этап парсера. (5) Server-side state — source of truth для disabled-условий UI, не local UI flag. (6) Любой нативный dialog в `platform/desktop/electron/*` — headless-guard обязателен; правило в CLAUDE.md давно есть, забыт при добавлении новой команды. (7) Catch-блок — это «понять или показать сырое», никогда не `console.warn(err)` без проброса в UI. (8) SQLite `busy_timeout` в любом read connection multi-connection setup — defaults = 0ms = немедленный fail. (9) При scheduling-операциях с overridable timeout — всегда отменять предыдущий handle.

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

**Где жило.** `platform/desktop/electron/settings-window.ts:153` (`isAutostartEnabled`) + `:177` (verify-readback в `setAutostartEnabled`).

**Root cause.** Асимметричный Electron Windows API: `app.setLoginItemSettings({ path, args, openAtLogin })` пишет в HKCU значение `"<exe>" --autostart`. `app.getLoginItemSettings()` **без аргументов** читает HKCU и сравнивает с дефолтным `process.execPath` (без args) — mismatch → возвращает `openAtLogin: false`, хотя запись физически есть. Это документированное поведение Electron, но non-obvious: написал с `{path, args}` → должен и читать с `{path, args}`. Симптом «set успешен, get возвращает false → verify фейлится → UI показывает error».

**Fix.** Константа `AUTOSTART_ARGS = ["--autostart"]` единственный источник. И `isAutostartEnabled()`, и verify-readback внутри `setAutostartEnabled` передают `{ path: process.execPath, args: AUTOSTART_ARGS }` в `getLoginItemSettings`. Симметрично с `set`.

**Регрешн-защита.** Defense-in-depth: одна константа гарантирует что изменение `args` потребует менять оба места одновременно. E2e на HKCU непрактично (Playwright бежит в test slot где autorunEnabled=false) — manual repro: prod-сборка → Settings → toggle → перезагрузка → проверить состояние тоггла и HKCU.

**Prevention.** **Когда Electron-API имеет парный `set`/`get`, проверяй симметричность сигнатур сразу.** Любая запись в реестр с non-default arguments должна верифицироваться чтением **с теми же** arguments. Применимо везде где есть `app.getXxx()` / `app.setXxx()` с optional parameters — `setUserTasks`, `setJumpList`, `setLoginItemSettings`.

### UPDATE 2026-05-28 — readback должен смотреть на фактические Windows launch items

**Симптомы.** Пользователь снова видит «Не удалось применить настройку» в Settings → «Автозапуск с Windows», при этом в `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` уже есть запись `Kosmos = "<...>\Kosmos.exe" --autostart`.
**Где жило.** `platform/desktop/electron/settings-window.ts` — `isAutostartEnabled()` полагался только на `openAtLogin` + legacy path probe, а UI делал второй `get()` после `set()`.
**Root cause.** Windows/Electron readback имеет больше состояния, чем один boolean: `getLoginItemSettings()` возвращает `launchItems[]` и `executableWillLaunchAtLogin`, а `openAtLogin` зависит от точного match'а `path`/`args`. После rename/install-path хвостов физически валидная HKCU Run запись может существовать под другим value-name или быть видна в `launchItems`, но UI всё равно покажет ошибку, если мы не сверяем фактический launch item.
**Fix.** `isAutostartEnabled()` теперь после симметричного `getLoginItemSettings({ path, args })` дополнительно проверяет `settings.launchItems[]`: enabled item с тем же normalized `process.execPath` и `["--autostart"]` считается валидным автозапуском даже если `openAtLogin` не совпал. Legacy cleanup расширен на `KeplerKosmos` / `KosmosKepler`, чтобы rename-хвосты не оставляли дубликаты. Settings UI переименован с Kepler на Kosmos.
**Регрешн-защита.** `bun run --cwd platform/desktop typecheck`, `bun run --cwd platform/desktop build:js`, `bun run docs:check`. На машине проверено, что HKCU Run содержит валидную запись `Kosmos = "<...>\Kosmos.exe" --autostart`.
**Prevention.** Для Windows autorun не считать `openAtLogin` единственным источником правды. Если API возвращает structured readback (`launchItems[]`), сверяй фактические path/args enabled items; value-name после rename/migration — metadata, а launchability определяется executable + args + StartupApproved state.

---

## 2026-05-23 — Kepler: launcher window не показывается при manual launch

**Симптомы.** При запуске `Kepler.exe` (через ярлык, exe, после установки NSIS, после autoupdater quit-and-install) launcher window не появляется — пользователь видит только tray icon, не понимает запустилось ли приложение вообще. (Пользователь жаловался на противоположный симптом — окно показывается при autostart — но фактический баг был симметричен: «всегда скрыто», т.е. оба пути сломаны.)

**Где жило.** `platform/desktop/electron/main.ts:1308-1310` в `app.whenReady().then(...)` — `createLauncher()` создавал окно с `show: false` и `launcherHidden = true`, но **ни одной ветки не было** которая бы дёргала `showLauncher()` при manual launch. Маркер `--autostart` уже выставлялся в `setAutostartEnabled` через args, но в `whenReady` им только логировали диагностику — поведение не зависело.

**Root cause.** Эволюционный артефакт. Изначально launcher был hidden-by-default + hotkey-only (стиль PowerToys Run / Spotlight). Но Kepler **также** имеет tray + меню запуска + ярлык — для пользователя, кликнувшего exe вручную, hidden-by-default UX-сломан (нет обратной связи). Маркер `--autostart` был добавлен для будущей дифференциации, но саму дифференциацию никто не дописал. Комментарий в `main.ts:1299` гласил «launcher по умолчанию hidden — это by design», что **закрепляло баг как намерение**.

**Fix.** Чистая функция `shouldShowLauncherOnStartup(argv): boolean` в `main.ts` — возвращает `!argv.includes("--autostart")`. После `createTray()` в `whenReady` вызываем `showLauncher()` если функция вернула true. Согласовано с `AUTOSTART_ARGS` в settings-window.ts (Bug «toggle автозапуска фейлится» fix зависит от того же маркера).

**Регрешн-защита.** `shouldShowLauncherOnStartup` — чистая функция от argv, экспортирована, тестируется без Electron mock'ов. Manual repro: `Kepler.exe` → окно показывается; `Kepler.exe --autostart` → tray-only.

**Prevention.** **Любая стартовая UX-логика разветвляющаяся по argv/env должна жить в маленькой чистой функции с явным test-surface** — иначе превращается в куски кода с шестью `if (process.env...)` ветками без проверок. Комментарии вида «by design» **без spec-ссылки** — красный флаг: если правило настоящее — оно в `docs-site/`, если нет — фоссилизированный артефакт, который маскирует bug как намерение.

---

## 2026-05-23 — Horologion: focus widget пропадает между фазами pomodoro

**Симптомы.** Pomodoro запущен, focus widget (docked корнер) показывается нормально, через ~25 минут (длительность work-фазы) **пропадает**. Pomodoro session при этом не stop'нута — `completed_pomodoros` уже инкрементнут, phase = ShortBreak (или LongBreak) с `isRunning=false`, ждёт ручного Skip/Start. Пользователь думает что pomodoro «всё ещё идёт», индикатора нет.

**Где жило.** `platform/desktop/electron/focus-widget.ts:446` (`deriveFocusStateFromBackend`, `widgetActive = isRunning && phase !== "idle"`) + `incubator/horologion/src/lib/usePomodoroSession.ts:147` (`pushFocusWidgetState`, `active = isRunning.value && !isPaused.value && phase.value !== "idle"`).

**Root cause.** Два разных контракта столкнулись. Backend session (`core/ark/crates/ark-core/rust/src/pomodoro/session.rs:399-438`, `finish_phase`) при `auto_start_break=false` (default config) после work делает `is_running=false`, эмитит `Finished`, затем переключает phase на ShortBreak/LongBreak с `is_running=false` и эмитит `PhaseChanged`. Backend трактует «не idle» = «session жива» (включая межфазный простой); widget derive трактовал «visible» как «активно тикает». При корректном backend-state `phase=ShortBreak, isRunning=false` widget уходил в hide, потеряв связь с реально живущей session. Единственный валидный индикатор «session закончилась» — `phase === "idle"`, потому что **только** `Session::stop()` возвращает phase в Idle.

**Fix.** В обоих местах деривации (main process + renderer push) `widgetActive` теперь `phase !== "idle"`. `phaseEndsAtMs` остаётся `null` пока `isRunning && !isPaused` ложен — автономный tick widget'а не запустится при межфазном простое, виджет покажет статичный MM:SS = `remainingSec`.

**Регрешн-защита.** `tests/e2e/horologion-focus-widget-between-phases.spec.ts` (headless Playwright): start pomodoro через ARK op → assert widget active → skip (work→ShortBreak с isRunning=false) → assert widget **всё ещё** active и BrowserWindow жив → stop → assert widget inactive. Без зависимости от реального 25-минутного wait.

**Prevention.** **Источник правды о видимости UI-элемента должен быть один и совпадать с lifecycle сущности, а не с её sub-state'ом.** «Сессия жива» (session.is_alive() = phase !== Idle) ≠ «активно тикает» (is_running). Если деривация одного UI-state'а живёт в двух местах (main derive + renderer push) — оба должны использовать **одну** чистую функцию, а не дублировать формулу. Кандидат: вынести `deriveWidgetActive(phase, isRunning, isPaused)` в `@kosmos/ark` или общий util и импортировать в обоих сайтах. Применимо ко всем UI-элементам которые tied к long-running backend state'у — focus widget, sync indicator, dashboard, любой tray badge.

---

## 2026-05-23 — Eden: drag-select прыгает по viewport

**Симптомы.** Mouse drag-select в Eden — viewport резко прокручивается вверх/вниз даже когда курсор далеко от края окна. Обычный in-view drag-select становится практически невозможным.

**Где жило.** `products/eden/src/Editor.css:28-29` + дубль `products/eden/src/App.css:2283-2284` — `scroll-padding-top: 30vh; scroll-padding-bottom: 30vh` на `.editor-wrapper`.

**Root cause.** Chromium во время mouse-drag для text-selection периодически вызывает native `scrollIntoView` для selection endpoint'а. `scroll-padding` участвует в расчёте видимости — что в padding-зоне считается «невидимым». При `padding: 30vh` любая selection в верхних или нижних 30% viewport'а триггерила scroll-into-view → viewport «догонял» selection прыжками. Не воспроизводится в jsdom (нет реализации selection scroll), не отлавливается через JS stack trace (вызов — из native Chromium).

**Fix.** Удалить `scroll-padding-top/bottom` из обоих селекторов `.editor-wrapper`. Breathing room «снизу при письме» сохранён через существующий `padding-bottom: 50vh` на `.ProseMirror` — это реальный layout, не виртуальный через scroll-padding. Autoscroll при выходе курсора за пределы окна (native Chromium + `useBlockSelection` для rubber-band) продолжает работать.

**Регрешн-защита.** Чистый CSS-фикс — JS-тест не нужен. В `Editor.css` оставлен комментарий объясняющий почему НЕ возвращать `scroll-padding`. Manual repro: открыть длинную заметку, поставить курсор в центр, drag-select мышью в пределах viewport — viewport должен оставаться стабильным.

**Prevention.** `scroll-padding` >> 0 на скролл-контейнере, содержащем contentEditable / textarea — **анти-паттерн**. Любой UA-инициированный scrollIntoView (drag-select, IME composition, find-in-page, accessibility focus) будет двигать viewport относительно padding-зоны, не относительно реальных краёв. Если нужно breathing room — используй реальный `padding-bottom` / spacer элемент.

### UPDATE 2026-05-28 — text drag у верхнего края снова уезжает вниз

**Симптомы.** При попытке выделить обычный текст в верхней части заметки Eden viewport снова резко скроллится вниз, хотя старый `scroll-padding: 30vh` уже удалён.
**Где жило.** `products/eden/src/Editor.vue:onContentMouseDown` запускает `useBlockSelection.startTracking` даже когда mousedown пришёл из текста внутри `.ProseMirror`; `products/eden/src/composables/useBlockSelection.ts:updateDrag` после пересечения границы блока активирует block-selection, блюрит TipTap, чистит native selection и включает auto-scroll.
**Root cause.** Block-selection был повешен на весь editor content area, а не только на gutter/пустые области. Обычный browser text-selection, начатый на тексте, при протягивании через соседний блок проходил `CROSSING_MIN_PX` и превращался в rubber-band block drag; дальше кастомный auto-scroll двигал `.editor-wrapper`, что пользователь видел как резкий уход вниз.
**Fix.** `onContentMouseDown` теперь проверяет `shouldStartBlockSelectionTracking`: block-selection стартует только из gutter/пустой editor surface, а mousedown по реальному тексту внутри `.ProseMirror` остаётся за browser-native text selection. DOM-gate вынесен в `products/eden/src/lib/blockSelectionPointer.ts`. `kepler-block-select-active` и `kepler-block-drag-active` разделены: `overflow:hidden` и `pointer-events:none` живут только на active drag, поэтому после mouseup persisted block selection больше не замораживает editor scroll. Edge-autoscroll delta вынесен в pure helper, cleanup теперь отменяет drag/rAF при unmount. TaskRef programmatic input focus при sibling navigation / autoFocus теперь тоже использует `{ preventScroll: true }`.
**Регрешн-защита.** `products/eden/tests/components/BlockSelectionPointer.spec.ts` проверяет, что paragraph/span/text-node/input внутри `.ProseMirror` не запускают block-selection tracking, а gutter/empty `.ProseMirror` surface всё ещё запускают. `BlockSelectionClasses.spec.ts` фиксирует split между persisted selection и active drag class. `BlockSelectionAutoScroll.spec.ts` фиксирует edge-zone autoscroll. Playwright smoke `.agent/tasks/2026-05-28-eden-scroll-drag-select/smoke/verify-eden-scroll-drag-select.mjs` грузит реальный `Editor.css` и проверяет: no large scroll-padding, persisted selection keeps `overflow-y:auto`, drag sets `overflow-y:hidden`, after drag returns to auto.
**Prevention.** Любой кастомный drag-layer поверх contentEditable должен иметь явный pointer gate: native text-selection владеет gesture, начатым на тексте, а кастомный selection-layer — только gutter/empty surface. Состояния “selection exists” и “drag is active” нельзя склеивать одним CSS class: scroll freeze, hover suppression и selection visuals имеют разные lifecycle. Threshold по движению не отличает “выделяю текст через соседний блок” от “выделяю блоки”, поэтому решать нужно на mousedown target, а не позднее в mousemove.

---

## 2026-05-23 — file_index UNIQUE constraint вешает backend

**Симптомы.** После часа работы Kepler внезапно подвисает: Eden показывает infinite loading, `commands.list` / `app_index.list_all` / `focus.*` все таймаутят через 30s. Backend.exe жив как процесс, supervisor не делает respawn, но WS-server не отвечает.

**Где жило.** `platform/runtime/src/file_index/store.rs::replace_all` (схема `files.path TEXT PRIMARY KEY` в `store.rs:14`). Вызов из `platform/runtime/src/file_index/mod.rs::rescan:102`, async-spawn из `platform/runtime/src/main.rs:299`.

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

**Регрешн-защита.** `platform/runtime/src/file_index/store.rs::tests::replace_all_dedupes_duplicate_paths` — кидает в `replace_all` три записи с двумя одинаковыми path'ами, проверяет что `Ok` и в БД ровно 2 строки.

**Prevention.**

- **Любой массовый bulk-insert по сырым данным из внешнего источника (FS scanner, network discovery, third-party API) обязан дедупиться перед `INSERT`.** SQLite UNIQUE — это контракт, который ты обещаешь соблюсти, не реактивный валидатор.
- **`std::sync::Mutex<Connection>` в долгих транзакциях, вызываемых из async task'а — анти-паттерн.** При 700k вставках держит worker thread tokio. Лучше: `tokio::task::spawn_blocking` или отдельный thread с channel'ом. См. [`db-resilience.md`](/concepts/db-resilience).
- **Backend должен писать stderr в файл, не только stdout.** Если stdout пропадает (zombie pipe), мы теряем все следы. Сейчас `crash_reporter` ловит panic, но stuck-без-panic — нет. Возможный TODO: периодический heartbeat-лог.
- **Supervisor должен делать health-check WS, а не только pid-alive.** Сейчас pid жив → supervisor спит, даже если WS-server stuck.

**Связанные правила.** [forbidden.md § Rust](/agents/forbidden) — про Mutex poison recovery, `RUST_BACKTRACE=1`, и обязательный `db_backup::maybe_backup_on_startup`. Этот случай показывает что недостаточно — нужен ещё health-check.
