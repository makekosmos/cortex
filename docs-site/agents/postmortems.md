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

## 2026-06-04 — ARK upsert удаляет object_links через SQLite REPLACE

**Симптомы.** Повторный upsert существующего объекта или типа мог незаметно удалить связи графа: обновление заметки/задачи/тега потенциально сносило `object_links`, которые на них ссылались.
**Где жило.** `crates/ark-core/rust/src/db.rs:609` (`upsert_object_type`), `crates/ark-core/rust/src/db.rs:875` (`upsert_object`), `crates/ark-core/rust/src/db.rs:1389` (`upsert_object_link`).
**Root cause.** В SQLite `INSERT OR REPLACE` реализован как delete старой строки плюс insert новой. Для `objects` и `object_types` это проходило через FK `ON DELETE CASCADE`, поэтому обычный upsert родительской строки мог каскадно удалить дочерние `object_links`/`objects`.
**Fix.** Generic object model helpers переведены на `INSERT ... ON CONFLICT(id) DO UPDATE SET ...`: `upsert_object_type`, `upsert_object` и `upsert_object_link` теперь обновляют существующую строку без delete+insert side effects.
**Регрешн-защита.** `cargo test --manifest-path crates\ark-core\rust\Cargo.toml --lib upsert_object` покрывает сохранение входящих/исходящих `object_links` при upsert объекта, сохранение objects/links при upsert object type и in-place update существующего link row.
**Prevention.** В SQLite `REPLACE` нельзя использовать как синоним upsert для таблиц с FK-зависимостями или потенциальными зависимыми строками. Для canonical ARK CRUD helper'ов upsert должен быть явным `ON CONFLICT DO UPDATE`, а регрессия должна проверять сохранение зависимых строк, не только итоговые поля родительской записи.

## 2026-06-01 — Dashboard usage показывает битые app icons

**Симптомы.** В Dashboard → «Затреканное время» часть строк показывала browser broken-image placeholder вместо иконки приложения.
**Где жило.** `shell/src/dashboard/store.ts` выбирал `RawTopAppEntry.iconRef` перед inline `app_index.list_all.icon_path`; `shell/src/dashboard/UsageTable.vue` рендерил `<img>` без fallback on-error.
**Root cause.** Usage analytics может вернуть stale/renderer-недоступный `tracked_apps.icon_ref` из usage-domain данных. При этом `app_index.list_all` уже специально инлайнит иконки как `data:image/png;base64,...`, но Dashboard отдавал приоритет `iconRef`, поэтому renderer пытался загрузить непригодный путь и показывал native broken-image icon.
**Fix.** Dashboard теперь выбирает иконку через `chooseUsageIconRef()`: сначала renderer-safe inline icon из `app_index.list_all`, затем только inline `data:image/*` из usage-domain; file/path-like `iconRef` без app-index fallback отбрасывается. `UsageTable` дополнительно скрывает `<img>` на `error`, чтобы единичная битая data-url не показывала browser placeholder.
**Регрешн-защита.** `bun shell/src/dashboard/store.regression.mjs`, `bun run --cwd shell typecheck`.
**Prevention.** Renderer-facing image fields должны быть либо уже inline/data URL, либо проходить через явный sanitizer/fallback. Нельзя смешивать storage/internal icon refs и browser-safe image src в одном приоритете: если endpoint специально инлайнит assets для renderer'а, UI должен предпочитать именно его.

## 2026-06-01 — Usage tracker спамит process window check failed

**Симптомы.** В логах `kepler-backend` каждую секунду повторялись строки `[usage-tracker] process window check failed for pid ...: Присоединенное к системе устройство не работает. (0x8007001F)` для одних и тех же PID.
**Где жило.** `services/kepler-backend/src/usage_tracker/mod.rs` в loop проверки `process_window_state`; Win32 probe живёт в `services/kepler-backend/src/usage_tracker/windows_capture.rs`.
**Root cause.** Tracker после ошибки Win32 probe классифицировал процесс как `AliveHidden`. Для умершего или transient-недоступного PID это оставляло `ActiveSession` в карте навсегда: runtime уже не рос, но каждый следующий tick снова вызывал тот же probe и снова писал ошибку в stderr.
**Fix.** Ошибка `process_window_state` теперь проходит через `process_window_state_or_dead`: если tracker не может подтвердить, что PID всё ещё принадлежит ожидаемому exe path, активная session считается `Dead`, финализируется и удаляется из `active_sessions`.
**Регрешн-защита.** `cargo test --manifest-path services\kepler-backend\Cargo.toml usage_tracker --lib` покрывает `process_window_probe_error_ends_session`: probe `Err("...0x8007001F")` должен возвращать `ProcessWindowState::Dead`.
**Prevention.** Для process accounting “не могу проверить identity процесса” нельзя трактовать как “процесс жив, но hidden”. Hidden допустим только после успешного чтения exe path и проверки совпадения identity; ошибки identity probe должны завершать session или иметь явный bounded retry.

## 2026-06-01 — Usage tracker занижает playtime игр

**Симптомы.** Dashboard показывал около 2 часов The Witcher 3, хотя пользователь реально играл примерно 10 часов. Read-only проверка `%APPDATA%\Kosmos\ark.db` подтвердила: недостающего времени нет ни в live DB, ни в WAL, ни в backup'ах.
**Где жило.** `services/kepler-backend/src/usage_tracker/mod.rs` держал один foreground-session state и завершал session при уходе с foreground окна; `services/kepler-backend/src/usage_tracker/windows_capture.rs` сэмплил только `GetForegroundWindow`.
**Root cause.** Usage tracker смешал две разные метрики: foreground activity и playtime/runtime процесса. Для игр пользователь ожидает «пока процесс игры жив», а код считал только «пока окно игры foreground». Дополнительно tracker task мог завершиться навсегда после одной Win32/DB ошибки, потому что `spawn()` логировал `FATAL`, но caller выбрасывал `JoinHandle`.
**Fix.** `usage_sessions` получил отдельный `runtime_ms` с additive migration/backfill из старого `foreground_ms + idle_ms`. Tracker теперь держит карту живых process sessions по `(tracked_app_id, pid)`, считает `runtime_ms` каждую секунду до завершения процесса, сохраняет session каждый tick и не завершает task из-за единичной capture/DB ошибки. Dashboard и game playtime aggregates используют `runtime_ms`, а `foreground_ms` оставлен отдельной диагностической метрикой.
**Регрешн-защита.** `cargo test --manifest-path crates\ark-core\rust\Cargo.toml usage_analytics_snapshot_includes_summary_and_zero_filled_trend`, `cargo test --manifest-path crates\ark-core\rust\Cargo.toml usage_game_playtime_summary_matches_bindings_and_range`, `cargo test --manifest-path crates\ark-core\rust\Cargo.toml test_init_schema_adds_usage_runtime_ms_to_existing_sessions`, `cargo test --manifest-path services\kepler-backend\Cargo.toml usage_tracker --lib`.
**Prevention.** В usage-домене нельзя использовать foreground как proxy для playtime. Для игр и долгоживущих приложений основная метрика — process runtime, foreground/idle — только дополнительные срезы. Любой бесконечный tracker loop должен переживать transient capture/persistence ошибки и иметь тест на отличие runtime от foreground.

## 2026-06-01 — Dashboard не показывает usage tracker

**Симптомы.** В Kepler Dashboard при открытии базы видны почти все пользовательские данные, но нет записей затреканных приложений / тайм-трекинга, хотя usage tracker пишет их в ARK DB.
**Где жило.** `shell/src/dashboard/store.ts` грузил только `list_object_types`, `list_objects` и `list_objects_by_type`; `shell/src/views/DashboardView.vue` строил sidebar только из object types.
**Root cause.** Dashboard был реализован как object browser поверх универсальной ARK object model. Usage tracker хранит данные в отдельных таблицах `tracked_apps`, `usage_sessions`, `usage_events`, которые не представлены в `object_types`, поэтому корректно записанные usage rows не могли появиться ни в sidebar, ни в таблице. При этом read-only aggregate endpoint `get_usage_analytics` уже существовал, но Dashboard его не использовал.
**Fix.** Dashboard получил отдельный sidebar-пункт «Затреканное время», который загружает `get_usage_analytics` и показывает aggregate по приложениям: название процесса, суммарное foreground-время, display name, число сессий, idle-время, последний запуск и normalized path. Object browser остался без raw SQLite и продолжает использовать ARK read-only IPC.
**Регрешн-защита.** `bun run --cwd shell typecheck`, `bun run format:check shell/src/dashboard/types.ts shell/src/dashboard/store.ts shell/src/dashboard/UsageTable.vue shell/src/views/DashboardView.vue docs-site/agents/postmortems.md`, `bun run docs:check`.
**Prevention.** Dashboard нельзя считать «полным просмотром базы», если он перечисляет только `object_types`. Для ARK-инспектора каждый non-object domain (`tracked_apps` / `usage_sessions` / `usage_events`, sync metadata, future side tables) должен иметь явный navigation surface или осознанно задокументированное исключение; иначе данные будут записываться корректно, но оставаться невидимыми в UI.

## 2026-05-30 — Focus widget pause ignores operation response

**Симптомы.** В focus widget кнопка «Пауза» не выглядела рабочей: клик уходил в backend, но сам виджет продолжал показывать running-состояние/кнопку «Пауза» и автономный тик мог продолжаться до следующего внешнего push.

**Где жило.** `shell/electron/focus-widget.ts:402` — `invokePomodoro()` вызывал `pomodoro.pause` / `pomodoro.resume`, но игнорировал returned state и полагался только на backend events.

**Root cause.** `PomodoroHost::pause()` и `PomodoroHost::resume()` в `services/kepler-backend/src/pomodoro_host.rs` intentionally не эмитят `pomodoro_phase_changed`: фаза не меняется, операция только меняет `isPaused`, `isRunning`, `remainingMs` и `phaseEndsAtMs`, возвращая новый snapshot в RPC response. Focus widget main-process path ожидал event-driven обновление для всех pomodoro ops, поэтому для pause/resume локальный `currentState` оставался stale.

**Fix.** `invokePomodoro()` теперь типизированно читает snapshot из `client.invokeOperation<PomodoroEventState>()` и сразу применяет `setFocusState(deriveFocusStateFromBackend(state))`. Event subscription остаётся для tick/phase_changed/finished, но inline controls больше не зависят от события там, где backend contract возвращает state response без event.

**Регрешн-защита.** `tests/e2e/focus-widget-controls.spec.ts` расширил pause test: после клика «Пауза» он проверяет появление кнопки «Продолжить», `focusWidget.getState().isPaused === true` и `phaseEndsAtMs === null`; затем кликает «Продолжить» и проверяет возврат к running state. Перед fix этот тест падал на отсутствии `.btn[aria-label="Продолжить"]`. После fix прошли `bun run --cwd shell typecheck`, `bun run format:check shell/electron/focus-widget.ts tests/e2e/focus-widget-controls.spec.ts docs-site/agents/postmortems.md`, `bun run test:e2e tests/e2e/focus-widget-controls.spec.ts`, `bun run --cwd shell build:js`.

**Prevention.** Для IPC/RPC controls нельзя предполагать, что mutation всегда придёт вторым событием. Если backend operation возвращает authoritative snapshot, UI-owner должен применить response немедленно; event stream — это синхронизация для внешних изменений и тиков, а не единственный способ обновить state после user action.

## 2026-05-30 — Focus block dev bundle loses dynamic imports

**Симптомы.** При `bun run --cwd shell dev` после старта Kepler в логах появлялись `[focus-block] service path failed: pingService is not a function` и `[extension-host] focus block apply failed: TypeError: isFocusServiceAutoInstallDeclined is not a function`. Focus-block fallback мог падать до helper/service path, хотя приложение продолжало запускаться.

**Где жило.** `shell/electron/focus-block.ts:169` и `shell/electron/focus-block.ts:218` — dynamic imports `./focus-service` / `./settings-window`; `shell/electron/main.ts` — Electron main entry экспортировал только `awaitArkReady` / `shouldShowLauncherOnStartup`.

**Root cause.** Vite/Rolldown code-splitting вынес `focus-block.ts` в отдельный dynamic chunk и переписал его dynamic imports модулей, уже попавших в main entry, в `import("./main.js")`. Но entry bundle не реэкспортировал нужные функции из `focus-service.ts` и `settings-window.ts`; на TypeScript уровне всё было валидно, а runtime module namespace в dev/prod bundle не содержал `pingService` / `isFocusServiceAutoInstallDeclined`.

**Fix.** `shell/electron/main.ts` теперь явно реэкспортит `getServiceStatus`, `runServiceCliElevated`, `pingService`, `sendViaPipe`, `isFocusServiceAutoInstallDeclined` и `setFocusServiceAutoInstallDeclined`. Это сохраняет текущую chunking-схему, но делает runtime namespace `main.js` совместимым с тем, во что bundler превращает dynamic imports из `focus-block.ts`.

**Регрешн-защита.** `bun run --cwd shell typecheck`, `bun run --cwd shell build:js`, `bun run format:check shell/electron/main.ts docs-site/agents/postmortems.md`, `bun run docs:check` прошли после fix. Дополнительно проверен собранный `shell/dist-electron/main.js`: export namespace содержит `pingService`, `sendViaPipe`, `isFocusServiceAutoInstallDeclined` и остальные API, которые импортирует `focus-block-*.js`.

**Prevention.** Если Electron main использует dynamic import для модулей, которые уже импортированы entrypoint'ом, обязательно проверять не только TypeScript, но и форму production/dev bundle. Code-splitting может переписать source-level imports на entry module namespace; тогда нужные функции должны быть entry exports либо модуль должен быть вынесен в отдельный shared chunk явно.

## 2026-05-28 — Focus widget label flicker and noisy controls

**Симптомы.** Pomodoro focus widget визуально дёргался при задаче: label мог прыгать между конкретным названием и дефолтным pomodoro label. Сам виджет был перегружен: controls постоянно занимали место вместо базового режима «время + задача».

**Где жило.** `shell/electron/focus-widget.ts` — `setFocusState` без различения качества источников сливал patches от backend events и renderer push. `shell/src/views/FocusWidgetView.vue` — controls были всегда видимыми при active session.

**Root cause.** Focus widget имеет два источника правды: backend events дают надёжный lifecycle/tick, а Horologion renderer знает более богатый UI context (draft title, tasks, focus blocking). Main process принимал последний patch целиком, поэтому более бедный backend tick мог перезаписать уже показанный конкретный label дефолтом. В UI controls были частью обычного flex layout, поэтому даже idle visual state выглядел как toolbar, а не compact status widget. Первый hover-only fix повесил reveal на область с `-webkit-app-region: drag`; в реальном Electron drag-region не ведёт себя как обычная DOM hover target, поэтому кнопки пропали и не раскрывались.

**Fix.** `setFocusState` теперь сохраняет уже известный конкретный label, когда следующий active patch в том же mode несёт только дефолтный `Фокус`/`Перерыв`. `FocusWidgetView` переведён в compact baseline: время и label занимают обычную no-drag hover area, а `IconButton` controls из `@kosmos/visuals` появляются на их месте поверх content overlay. Перетаскивание вынесено в отдельную `.drag-handle` с `GripVertical`, окно стало уже, левая accent-полоса удалена, а mode-индикация стала заполняющим progress-fill внутри плашки через новый `totalSec` в focus widget state.

**Регрешн-защита.** `tests/e2e/focus-widget-controls.spec.ts` добавил regression `backend generic label не перетирает конкретное название` и проверяет, что controls скрыты до hover/focus, content-zone `no-drag`, handle-zone `drag`, а pause/skip/stop остаются кликабельны. `bun run --cwd shell typecheck`, `bun run format:check ...`, `bun run --cwd shell build:js`, `bun run test:e2e tests/e2e/focus-widget-controls.spec.ts` прошли после fix.

**Prevention.** Для state, который собирается из нескольких источников разной полноты, merge обязан учитывать качество поля, а не только “последний patch победил”. Backend lifecycle/tick может быть authoritative по времени, но не по UI context; renderer-owned label/task context нельзя затирать дефолтами из более бедного события. В Electron floating widgets нельзя совмещать hover-only controls с `-webkit-app-region: drag` на той же DOM-зоне; drag должен жить в отдельной маленькой области, а hover/click controls — в `no-drag`.

## 2026-05-27 — Akasha EPUB blocks overlap on resize and miss source semantics

**Симптомы.** При сужении/расширении окна Akasha расстояния между абзацами не пересчитывались, текст начинал налезать друг на друга, а часть EPUB-стилей из реальной книги выглядела как обычный плоский текст. Копирование выделения было неочевидным.

**Где жило.** Старый GPUI reader Akasha — virtual-list item heights оценивались по фиксированной ширине `70ch` и не зависели от viewport width. Parser учитывал только базовые `strong/em`, но не EPUB semantic spans вроде `epub:type="bridgehead"` и inline tags вроде `cite`.

**Root cause.** Виртуализация получила стабильные item sizes, но эти sizes описывали “идеальную” ширину, а не текущую ширину reader column; при wrap на узком окне GPUI рисовал больше строк, чем virtual list зарезервировал места. Семантические EPUB-теги терялись до UI-слоя, поэтому Apple Books tokens не могли примениться к ним визуально.

**Fix.** Akasha теперь хранит текущую ширину reader column и пересчитывает `reader_item_sizes` при изменении `window.viewport_size()`, поэтому virtual-list rows растут вместе с переносами текста. Paragraph/list blocks получили Apple Books-style vertical rhythm через em-based bottom margins. Parser теперь сохраняет `epub:type="bridgehead"` как bold и `cite` как emphasis. После drag-selection появляется маленькая floating-подсказка с `Copy`, которая вызывает стандартный `TextView` copy action.

**Регрешн-защита.** `cargo test -p akasha` добавил regression tests на responsive item-height (`estimated_block_height_tracks_reader_width`), paragraph gap (`paragraph_estimate_includes_apple_books_gap`) и EPUB semantic inline tags (`preserves_epub_bridgehead_and_cite_semantics`). `cargo check -p akasha`, `bun run --cwd shell typecheck`, `bun run ark:guard:writes` прошли после fix.

**Prevention.** Для virtualized reader/feed UI item estimate должен принимать тот же layout width, при котором реально рисуется текст; фиксированные `ch`-оценки допустимы только для фиксированной колонки. EPUB parser обязан сохранять не только HTML-теги, но и распространённые semantic attributes (`epub:type`) до UI-слоя, иначе typography tokens применяются к уже обеднённой модели.

## 2026-05-27 — Akasha reader без scrollbar, незаметное открытие книги и медленный native dev launch

**Симптомы.** В Akasha не было видимого scrollbar у reader'а, повторное открытие другой книги было неочевидным, а запуск native Akasha из Kepler в dev ощущался медленным.

**Где жило.** Старый GPUI reader Akasha — `v_virtual_list` использовался без отдельного `Scrollbar::vertical`, а кнопка открытия была вторичной в toolbar. `extensions/akasha/manifest.json` и `shell/scripts/build-extensions.mjs` — dev native path/build указывали на debug binary.

**Root cause.** `gpui_component::v_virtual_list` виртуализует scroll surface, но не рисует scrollbar автоматически. Для native extension dev flow был выбран `target/debug/akasha.exe`, что удобно для отладки, но плохо совпадает с ожиданием “открывается как приложение”.

**Fix.** Reader virtual-list теперь обёрнут в `relative` container с `Scrollbar::vertical(&reader_scrollbar)`. Кнопка открытия книги вынесена первой в toolbar и переименована в «Открыть книгу». Native dev executable переключён на `target/release/akasha.exe`, а `build:extensions` собирает native extensions через `cargo build --release -p <pkg>`.

**Регрешн-защита.** `cargo check -p akasha` проверяет scrollbar/layout интеграцию; `cargo test -p akasha` проверяет reader state/parser; `bun run --cwd shell typecheck` проверяет manifest/native host types.

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

### UPDATE 2026-05-28 — readback должен смотреть на фактические Windows launch items

**Симптомы.** Пользователь снова видит «Не удалось применить настройку» в Settings → «Автозапуск с Windows», при этом в `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` уже есть запись `Kosmos = "<...>\Kosmos.exe" --autostart`.
**Где жило.** `shell/electron/settings-window.ts` — `isAutostartEnabled()` полагался только на `openAtLogin` + legacy path probe, а UI делал второй `get()` после `set()`.
**Root cause.** Windows/Electron readback имеет больше состояния, чем один boolean: `getLoginItemSettings()` возвращает `launchItems[]` и `executableWillLaunchAtLogin`, а `openAtLogin` зависит от точного match'а `path`/`args`. После rename/install-path хвостов физически валидная HKCU Run запись может существовать под другим value-name или быть видна в `launchItems`, но UI всё равно покажет ошибку, если мы не сверяем фактический launch item.
**Fix.** `isAutostartEnabled()` теперь после симметричного `getLoginItemSettings({ path, args })` дополнительно проверяет `settings.launchItems[]`: enabled item с тем же normalized `process.execPath` и `["--autostart"]` считается валидным автозапуском даже если `openAtLogin` не совпал. Legacy cleanup расширен на `KeplerKosmos` / `KosmosKepler`, чтобы rename-хвосты не оставляли дубликаты. Settings UI переименован с Kepler на Kosmos.
**Регрешн-защита.** `bun run --cwd shell typecheck`, `bun run --cwd shell build:js`, `bun run docs:check`. На машине проверено, что HKCU Run содержит валидную запись `Kosmos = "<...>\Kosmos.exe" --autostart`.
**Prevention.** Для Windows autorun не считать `openAtLogin` единственным источником правды. Если API возвращает structured readback (`launchItems[]`), сверяй фактические path/args enabled items; value-name после rename/migration — metadata, а launchability определяется executable + args + StartupApproved state.

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

### UPDATE 2026-05-28 — text drag у верхнего края снова уезжает вниз

**Симптомы.** При попытке выделить обычный текст в верхней части заметки Eden viewport снова резко скроллится вниз, хотя старый `scroll-padding: 30vh` уже удалён.
**Где жило.** `extensions/eden/src/Editor.vue:onContentMouseDown` запускает `useBlockSelection.startTracking` даже когда mousedown пришёл из текста внутри `.ProseMirror`; `extensions/eden/src/composables/useBlockSelection.ts:updateDrag` после пересечения границы блока активирует block-selection, блюрит TipTap, чистит native selection и включает auto-scroll.
**Root cause.** Block-selection был повешен на весь editor content area, а не только на gutter/пустые области. Обычный browser text-selection, начатый на тексте, при протягивании через соседний блок проходил `CROSSING_MIN_PX` и превращался в rubber-band block drag; дальше кастомный auto-scroll двигал `.editor-wrapper`, что пользователь видел как резкий уход вниз.
**Fix.** `onContentMouseDown` теперь проверяет `shouldStartBlockSelectionTracking`: block-selection стартует только из gutter/пустой editor surface, а mousedown по реальному тексту внутри `.ProseMirror` остаётся за browser-native text selection. DOM-gate вынесен в `extensions/eden/src/lib/blockSelectionPointer.ts`. `kepler-block-select-active` и `kepler-block-drag-active` разделены: `overflow:hidden` и `pointer-events:none` живут только на active drag, поэтому после mouseup persisted block selection больше не замораживает editor scroll. Edge-autoscroll delta вынесен в pure helper, cleanup теперь отменяет drag/rAF при unmount. TaskRef programmatic input focus при sibling navigation / autoFocus теперь тоже использует `{ preventScroll: true }`.
**Регрешн-защита.** `extensions/eden/tests/components/BlockSelectionPointer.spec.ts` проверяет, что paragraph/span/text-node/input внутри `.ProseMirror` не запускают block-selection tracking, а gutter/empty `.ProseMirror` surface всё ещё запускают. `BlockSelectionClasses.spec.ts` фиксирует split между persisted selection и active drag class. `BlockSelectionAutoScroll.spec.ts` фиксирует edge-zone autoscroll. Playwright smoke `.agent/tasks/2026-05-28-eden-scroll-drag-select/smoke/verify-eden-scroll-drag-select.mjs` грузит реальный `Editor.css` и проверяет: no large scroll-padding, persisted selection keeps `overflow-y:auto`, drag sets `overflow-y:hidden`, after drag returns to auto.
**Prevention.** Любой кастомный drag-layer поверх contentEditable должен иметь явный pointer gate: native text-selection владеет gesture, начатым на тексте, а кастомный selection-layer — только gutter/empty surface. Состояния “selection exists” и “drag is active” нельзя склеивать одним CSS class: scroll freeze, hover suppression и selection visuals имеют разные lifecycle. Threshold по движению не отличает “выделяю текст через соседний блок” от “выделяю блоки”, поэтому решать нужно на mousedown target, а не позднее в mousemove.

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
