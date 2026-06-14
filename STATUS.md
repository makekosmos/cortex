# Kosmos — статус проекта (2026-06-15)

## 2026-06-15 — Prod/dev source inversion fix (Kosmos Desktop 0.4.9 → 0.4.10)

Patch-релиз исправляет классификацию built-in extensions: packaged
`resources/extensions` больше не помечается как `source: dev`, поэтому Eden и
другие bundled extensions снова идут как production/bundled copy, а не как
репозиторный dev-tree. Из-за этого исчезают ложные `источник: репозиторий` /
`dev` бейджи, и marketplace/update flow снова видит установленный Eden как
обычный installed target вместо shadowed dev-версии.

## 2026-06-15 — Tray icon repack (Kosmos Desktop 0.4.8 → 0.4.9)

Patch-релиз перевыпускает desktop shell после того, как pipeline сборки трея
стал генерировать `tray.ico` из `tray.svg`. Это гарантирует, что новая иконка
попадает в packaged app как `resources/tray.ico`, а не остаётся на старом
артефакте.

## 2026-06-14 — Stage 2 release bump

Kosmos Desktop: 0.4.6 → 0.4.7. Eden: 0.2.0 → 0.2.1.
Патч-релиз закрывает уже готовые пользовательские изменения: новые app/tray
icons из `platform/desktop/build` и более плотный sidebar/object layout в Eden.

## 2026-06-12 — Eden Obsidian vault + CodeMirror polish (Eden 0.1.12 → 0.2.0)

Minor release Eden закрывает большой pass по редактору и переносимости заметок.

- **Obsidian vault import/export**: Eden умеет импортировать папку `.md` заметок
  с YAML frontmatter и `[[wikilinks]]`, а также экспортировать заметки обратно в
  Markdown-файлы, пригодные для vault workflow.
- **Изображения как объекты**: добавлен системный тип `image_obj`; импорт vault
  подхватывает локальные изображения, сохраняет размер/разрешение и показывает
  Markdown-картинки в CodeMirror preview.
- **CodeMirror/Vim editor hardening**: старые TipTap/ProseMirror JSON-записи
  нормализуются на entry-boundary, поэтому записи вроде `Мое пространство` и
  дневниковых дат больше не проваливаются в legacy editor без Vim/cursor UX.
- **Sidebar и focus polish**: Eden использует settings-like sidebar/chrome из
  `@kosmos/visuals`, выбранные пункты получили читаемый background state, zen
  mode показывает title в titlebar.
- **Scroll fix перед релизом**: CodeMirror `scrollDOM` снова единственный
  vertical scroll container; длинные заметки скроллятся колесом/тачпадом внутри
  текста.

Proof loop: `.agent/tasks/2026-06-12-eden-obsidian-vault/`.

Checks: `bun test tests/cmGate.test.ts`, `bun test tests/obsidianVault.test.ts`,
`bunx vitest run tests/components/CmConvert.spec.ts tests/systemTypes.test.ts --config vite.config.mjs`,
`node scripts/check-ark-write-boundaries.mjs`, `node scripts/build-extensions.mjs --only eden`,
`node scripts/check-docs-freshness.mjs`, `git diff --check`.

## 2026-06-10 — App icons refresh (Kosmos Desktop 0.4.5 → 0.4.6)

Патч-релиз обновляет набор иконок приложения с повышенной четкостью и дополнительными форматами.

- **Tray icon multi-size**: отдельная иконка трея (tray.ico) с размерами 16–48 пиксель, отрендерена из вектора tray.svg.
- **High-res app icon**: иконка приложения в высоком разрешении 1000×1000 пиксель для Retina-дисплеев.
- **Installer icon**: перегенерированный icon.ico инсталлера с использованием фильтра Lanczos для лучшей интерполяции.

## 2026-06-10 — Electron main-process async I/O + app icon (Kosmos Desktop 0.4.4 → 0.4.5)

Патч-релиз устраняет синхронный I/O на горячих путях main process, добавляет новую иконку приложения.

- **Async I/O on main hot paths**: installer, diagnostics, autostart, kosmos-icon protocol, marketplace — переведены на `fs.promises` и асинхронный `spawn` вместо `*Sync` вызовов. Proof loop `.agent/tasks/2026-06-10-electron-main-async-io/`, новый regression test `no-sync-io.test.ts`.
- **No window hangups on extension operations**: неблокирующий async I/O устранил провисания всех окон при установке/откате расширений, сборке bug-report bundle, переключении автозапуска, чтении иконок приложений.
- **New app icon**: обновлена иконка Kosmos Desktop.
- **WIP follow-ups**: app-index throttling/ranking improvements, window-effects flat flag, docs-site/storybook maintenance.

## 2026-06-09 — Performance hardening + startup fixes (Kosmos Desktop 0.4.3 → 0.4.4)

Патч-релиз закрывает накопившиеся perf-регрессии и диагностические улучшения бэкенда.

- **Background-maintenance diagnostics**: Phase 3 диагностики бэкенда — WPR trace, structured telemetry для maintenance-сканов.
- **ARK backup non-blocking**: `db_backup::maybe_backup_on_startup` переведён на background-priority chunked запись, не блокирует UI-старт.
- **Launcher scroll/reflow**: устранён лишний reflow списка при повторном открытии лаунчера; smoother scroll при большом количестве команд.
- **Startup maintenance priority**: background-сканы (integrity check, vacuum stats) запускаются на фоновом приоритете, не задерживают ready-сигнал.
- **Launcher focus on Windows**: лаунчер корректно захватывает foreground и keyboard focus при вызове hotkey.

## 2026-06-07 — Bridge release на makekosmos channels (Kosmos Desktop 0.4.2 → 0.4.3)

Kosmos Desktop получил миграционный release для переноса update/distribution
каналов в отдельную GitHub organization `makekosmos`.

- Новый primary desktop channel: `makekosmos/desktop`. Он идёт первым в
  `platform/desktop/package.json → build.publish`, чтобы установленная сборка
  0.4.3 дальше проверяла обновления уже из makekosmos.
- Legacy bridge channel: `yoso-industries/kepler-releases`. Тот же artifact
  0.4.3 публикуется туда на переходный период, чтобы уже установленные версии,
  которые ещё смотрят в старый updater repo, получили миграционное обновление.
- Extension marketplace source-of-truth перенесён на `makekosmos/extensions`:
  `extension-marketplace.ts → CATALOG_URL`, `publish-extension.mjs` и
  `generate-catalog.mjs → RELEASES_REPO`.
- Distribution docs, system requirements, forbidden rules и manual download link
  обновлены под новые каналы. Legacy `yoso-industries/kepler-releases` описан
  как временный bridge target, который можно убрать после переходного окна.

Proof loop: `.agent/tasks/2026-06-07-makekosmos-release-channel/`.

## 2026-06-06 — Clipboard history заморожена + perf hardening (Kosmos Desktop 0.4.1 → 0.4.2)

Фича «История буфера обмена» снята с глаз во всех сборках (dev и prod) — фоновый
поллинг clipboard давал провисания ввода и сопутствующий coil-whine на нагруженной
системе (репорт пользователя: провис при копировании текста; «бип» из колонок при
100% CPU). Диагностика подтвердила два источника, оба устранены в коде, но фича
всё равно заморожена до доработки поллинга.

- Единый рубильник `CLIPBOARD_HISTORY_ENABLED` в `platform/desktop/shared/ipc-types.ts`
  (`false`). Им загейтлены: запуск поллинга/IPC/shell-opener (`main.ts`), команда
  лаунчера «Открыть буфер обмена» (`commands.ts`), вкладка настроек «Буфер обмена»
  (`settings/navigation.ts`). Режим `clipboard` в `LauncherView.vue` остался в
  коде, но недостижим (нет точек входа).
- Первопричина провиса: `detectClipboardSource` запускал на КАЖДОЕ копирование
  синхронный `powershell.exe` + `Add-Type` (компиляция C#). Определение источника
  удалено полностью.
- Картинка PNG-кодируется (`toDataURL`) только при смене отпечатка raw-битмапа
  (`fingerprintImageBytes`), а не каждые 800 мс.
- `commit` в store больше не сериализует всю историю трижды; запись на диск
  асинхронная с дебаунсом; лимит на размер одной записи 10 МБ; дефолт истории
  512 → 64 МБ.
- Док: `docs-site/concepts/clipboard-history.md` (статус, причина, условия
  расфриза) + ссылка в сайдбаре.

Proof loop: `.agent/tasks/2026-06-06-clipboard-history-jank/`.

Checks: `bun run shell:typecheck`, `bun test platform/desktop/electron/clipboard-history-store.test.ts`
(6/6), `bun run ark:guard:writes`, `bun run docs:check`, визуальная проверка в dev
(команда и вкладка скрыты, провисов при копировании нет).

## 2026-06-06 — Idle CPU and dev startup hardening (Kosmos Desktop 0.4.0 → 0.4.1)

Kosmos Desktop получил patch release после диагностики фоновой нагрузки и
повторных dev-запусков.

- `usage_tracker` больше не пишет `tracked_app`/`usage_session` в ARK на каждый
  poll стабильного foreground окна. Runtime копится in-memory, heartbeat
  сохраняет `usage_session` не чаще раза в минуту, а `tracked_app.last_seen_at`
  фиксируется при завершении session. На dev DB это убрало постоянный rewrite
  `lan_sync.version_vector` и снизило idle CPU `kepler-backend`/`ark-core-rpc`
  до нулевого прироста в 70-секундном clean dev замере.
- Recursive file-index watcher больше не включается по умолчанию на широких
  persisted roots. Startup/manual rescan остаётся, а постоянный recursive watch
  требует явного `KEPLER_FILE_INDEX_WATCHER=1`.
- `bun run --cwd platform/desktop dev` теперь заранее проверяет dev-порты platform/desktop/extension
  (`5173`, `5185` для Akasha default-on) и останавливается с понятным сообщением,
  если прошлый dev-run ещё жив. Shell Vite закреплён на `127.0.0.1:5173` со
  `strictPort`, поэтому он больше не уезжает молча на `5174`.
- Убран stale `SubmitError::InjectJoin`, который давал dead-code warning в
  `kepler-backend` build.

Proof loop: `.agent/tasks/2026-06-06-backend-cpu-loop/`.

Checks: `cargo fmt --package kepler-backend`,
`cargo test -p kepler-backend active_session_heartbeat_flush_is_rate_limited -- --nocapture`,
`cargo test -p kepler-backend recursive_watcher_is_opt_in -- --nocapture`,
`node --check platform/desktop/scripts/dev.mjs`, `node platform/desktop/scripts/dev.mjs` with busy
ports, clean `bun run --cwd platform/desktop dev`, `bun run ark:guard:writes`,
`bun run docs:sync`, `bun run docs:check`, `bun run ark:smoke`.

## 2026-06-06 — Raycast-style command surfaces and Focus hardening (Kosmos Desktop 0.3.12 → 0.4.0)

Kosmos Desktop получил крупный shell release: launcher стал ближе к Raycast по
модели команд, clipboard/focus переехали в command surfaces, а Focus mode
получил app-blocking UX поверх запуска приложений из Kosmos.

- Добавлен compatibility runtime для Raycast-like commands: host умеет запускать
  command views, держать per-command session snapshots и рендерить List, Grid,
  Detail, Form и Menu Bar style surfaces через встроенный renderer.
- `@raycast/api` workspace package получил JSX/runtime bridge, navigation,
  action panels, forms, dropdown/tag/file picker flows, keyboard shortcuts,
  local storage и view-model тесты. Это foundation для будущих Kosmos/Raycast
  compatible extensions без прямого доступа к Electron internals.
- Clipboard history теперь живёт как Shell command surface: отдельный режим
  launcher'а показывает текст, ссылки, цвета, файлы и изображения, поддерживает
  фильтр типов, copy/open/delete/pin/clear и layout в стиле Raycast.
- Focus command surface унифицирован с clipboard: форма открывается внутри
  Shell, стартует сессию без лишней loading-паузы, сохраняет цель и Delphi task
  через `@` mention, а launcher больше не теряет keyboard focus после переходов.
- Во время активной focus-сессии launcher показывает state-aware команды вместо
  сырого command-bus списка: «Приостановить/Продолжить», «Отметить задачу
  выполненной», «Завершить», «Редактировать». «Начать фокус» видна только в idle.
- Focus app blocking теперь использует launcher-facing identity: выбранные
  приложения отображаются как user-facing chips, запуск заблокированного
  приложения через Kosmos перехватывается, а full-screen overlay даёт короткий
  snooze без выключения всей сессии.
- Backend focus active state хранит `blocked_apps` metadata рядом с ids, а shell
  process watcher закрывает уже запущенные/новые заблокированные процессы без
  privileged hosts writes вне helper/service boundary.
- Документация обновлена по command bus, extension host, focus mode, архитектуре
  и app map; proof-loop evidence сохранён для Raycast host, clipboard parity,
  focus mode unification, app mentions/blocking и focus launcher commands.

Proof loops:

- `.agent/tasks/2026-06-04-raycast-compat-runtime/`
- `.agent/tasks/2026-06-05-clipboard-history-parity/`
- `.agent/tasks/2026-06-05-focus-mode-unification/`
- `.agent/tasks/2026-06-05-focus-app-mentions/`
- `.agent/tasks/2026-06-05-focus-blocking-ux/`
- `.agent/tasks/2026-06-05-focus-launcher-commands/`

Checks: `bun test tests/unit/clipboard-history-store.test.ts tests/unit/focus-app-blocking.test.ts tests/unit/focus-command-instant-render.test.ts tests/unit/focus-command-payload.test.ts tests/unit/focus-launcher-commands.test.ts tests/unit/launcher-commands.test.ts tests/unit/raycast-api.test.ts tests/unit/raycast-command-runner.test.ts tests/unit/raycast-manifest.test.ts tests/unit/raycast-view-model.test.ts`,
`bun run --cwd platform/desktop typecheck`, `bun run ark:guard:writes`,
`bun run --cwd platform/desktop build:js:shell`, `bun run docs:sync`, `bun run docs:check`.

Большой hardening/release pass после review всего worktree.

- Runtime permissions для user-installed Vue extensions теперь enforce'ятся в
  main-process IPC: ARK requests, ARK event subscriptions, userData и
  focus-widget state больше не являются documentation-only surface.
- First-party trust стал source-based (`dev` / `bundled`), а не id-based:
  user-installed override с id вроде `eden` не получает права встроенного
  приложения.
- `@kosmos/ark::invokeOperation()` нормализует `device_id` для raw local writes,
  чтобы sync HLC/version vector не принимал spoofed renderer payload.
- Delphi runtime очищен от legacy spaces/P2P/local JSON/local DB graph:
  extension сразу открывается в task UI и грузит задачи из single ARK DB.
- Dictation режет long-form WAV на ≤30s chunks перед Groq/Whisper transport,
  поэтому речь после первых 30 секунд не теряется. Settings autostart toggle
  больше не показывает ложную ошибку из-за immediate Windows readback lag.
- Akasha EPUB parser получил budget guardrails на исходный файл, ZIP entries,
  uncompressed payload, cover и количество reader blocks.
- ARK legacy/usage/pending/sync_kv upserts переведены с SQLite `REPLACE` на
  `ON CONFLICT DO UPDATE`, чтобы не получать delete+insert side effects.
- Headless/test repeat-open guards закрывают повторное `.focus()` / `.show()`
  для Settings, Dashboard и install dialog.
- Review fix: `kepler:focus-widget:set-state` снова принимает first-party shell
  renderer calls, но продолжает проверять `focus.control` для extension senders.
- Repo hygiene: `.agent` runtime artifacts игнорируются, root `Cargo.lock`
  зафиксирован для binary workspace, Bun packageManager выровнен на `1.3.14`,
  icon sources ужаты до 1024×1024, docs freshness guard проверяет root docs.
- Agent skills updated: `.agents/skills/windows-sandbox/` добавлен как runbook
  по повторяющимся Windows Sandbox failures; `bump` skill уточняет patch-only
  default и обязательные build+publish шаги.

Proof loops:

- `.agent/tasks/2026-06-04-extension-permissions-consolidation/`
- `.agent/tasks/2026-06-04-dictation-autostart-bugs/`
- `.agent/tasks/2026-06-04-full-review-bump-release/`

Checks: `bun test tests/unit/extension-permissions.test.ts tests/unit/settings-autostart-ui.test.ts tests/unit/ark-client-invoke-device-id.test.ts tests/unit/akasha-epub-guardrails.test.ts`,
`bun run --cwd platform/desktop typecheck`, `bun run shell:build`,
`bun run --cwd platform/desktop build:extensions`, `bun run docs:sync`,
`bun run docs:check`, `bun run ark:guard:writes`, `bun run ark:smoke`,
`bunx playwright test --config playwright.config.ts tests/e2e/extension-permissions.spec.ts tests/e2e/headless-window-repeat-open.spec.ts tests/e2e/delphi-legacy-cleanup.spec.ts tests/e2e/extensions-contract.spec.ts tests/e2e/commands-architecture.spec.ts tests/e2e/focus-widget-controls.spec.ts`,
`git diff --check`.

## 2026-06-04 — Delphi on Kosmos visuals (Delphi 0.1.5 → 0.1.6, Kosmos Desktop 0.3.10 → 0.3.11)

Delphi визуально переведён на settings-like chrome из `@kosmos/visuals`.

- Root окна теперь использует `DesktopChrome appearance="settings"`: прозрачный
  titlebar без нижнего бордера и без лишних кнопок настроек / скрытия sidebar.
- Sidebar собран на `SettingsSidebar` / `SettingsSidebarButton`, получил более
  плотный Mica/Acrylic-тон и отключённое выделение текста на навигационных
  кнопках.
- Иконки навигации Delphi переведены на Phosphor Icons без градиентной
  подложки: duotone в обычном состоянии, fill в активном.
- Окно Delphi объявляет `windowEffect: "acrylic"`, а host теперь явно вызывает
  `BrowserWindow.setBackgroundMaterial()` для extension windows, чтобы Mica /
  Acrylic применялись как в Settings.
- Быстрое создание задачи больше не даёт белую foreground-тень в тёмной теме.
- `@kosmos/visuals` получил расширения для переиспользования этого паттерна:
  `DesktopChrome.appearance`, прозрачный `Titlebar`, `SettingsSidebar.tone` и
  plain-вариант иконок в `SettingsSidebarButton`.

Proof loop: `.agent/tasks/2026-06-04-delphi-kosmos-visuals/`.

## 2026-06-03 — Akasha GPUI extracted, Vue reader in Kosmos (Akasha 0.1.0 → 0.1.1)

Akasha больше не живёт как Rust/GPUI app внутри Kosmos workspace.

- Старый GPUI-код Akasha вынесен в приватный standalone repo
  `ksanrse/akasha-gpui`.
- Бывший workspace Akasha удалён из Cargo workspace; current Kosmos reader живёт в `incubator/akasha`.
- `incubator/akasha` переведён на обычный Vue-extension contract:
  `kind: "vue"`, `entryHtml: "dist/index.html"`, `devPort: 5185`.
- Новый reader открывает `.epub` через file input, читает ZIP/OPF/spine в
  renderer'е, рендерит главы непрерывным потоком и сохраняет настройки чтения в
  `extensions-data/akasha/reader-state.json`.
- Reader получил локальную библиотеку без магазина: EPUB импортируется в
  `extensions-data/akasha/books/<sha256>.epub`, `library.json` хранит карточки
  книг, а позиция чтения сохраняется как `chapterId` + `blockId` + процент.
- EPUB XHTML не рендерится через `v-html`; parser превращает его в безопасные
  `ReaderBlock` / `InlineSpan`.

Proof loop: `.agent/tasks/2026-06-03-akasha-vue-migration/`.

## 2026-06-03 — Visuals Tailwind scale pass (Kosmos Desktop 0.3.9 → 0.3.10)

`@kosmos/visuals` переведён на Tailwind v4 как implementation layer для shared
Vue primitives, но public contract остался через CSS variables и public exports.

- `theme/css-variables.css` теперь явно фиксирует layout scale: большие
  размеры/отступы на 8px multiples, мелкие детали допускают 2px/4px, typography
  живёт на отдельной rem-шкале.
- Shared controls выровнены под predictable dimensions: 24/32/40px controls,
  32/40/48px rows, tokenized radii `--radius-card`, `--radius-button`,
  `--radius-input`.
- Storybook 10 теперь грузит Tailwind через `@tailwindcss/vite` и отдельный
  `theme/storybook.css`, а не вручную подтягивает старые component CSS entry.
- Починен Storybook/Bun/Windows resolution: framework идёт package specifier'ом,
  `vue-component-type-helpers@2.2.12` добавлен как explicit visuals dev dep.
- Починены broken composed stories `DesktopChrome` и `Titlebar in window`, где
  Storybook decorator рендерил `[object Object]`, а pattern использовал старый
  `#titlebar` slot вместо `#titlebar-leading/center/trailing`.

Checks: `bun run --cwd packages/visuals format:check`,
`bun run --cwd packages/visuals test`, `bun run --cwd packages/visuals build-storybook`,
`bun run --cwd platform/desktop typecheck`, `bun run --cwd platform/desktop build:extensions`,
Playwright screenshots of Dropdown / Toggle / SettingsRow / DesktopChrome /
Titlebar-in-window stories under `.tmp/visuals-check/`.

## 2026-06-02 — Dashboard table visual polish (Kosmos Desktop 0.3.8 → 0.3.9)

Dashboard «Таблица данных» получил визуальный pass после реального просмотра
через Playwright screenshots:

- левая навигация переехала на settings primitives из `@kosmos/visuals`, без
  legacy content-подложки и отдельной страницы «Настройки»;
- тип `blocklist_obj` скрыт из Dashboard: focus blocklists — это настройки
  Focus mode, а не пользовательские объекты таблицы данных;
- object table показывает `иконка / название / последняя модификация`, а
  `note_obj`, `system-type-journal`, `time_entry_obj`, `game_obj`, `tag_obj`,
  `task_obj` получили отдельные Phosphor duotone/fill иконки и глубокие
  контрастные цвета;
- usage table стала плотнее и ближе к Linear-style спискам: заголовки отделены
  от scrollable body, scrollbar не заезжает на header, первая колонка
  выровнена по названиям приложений, а не по icon-slot;
- Dashboard кеширует загруженные object types, object rows по type id и usage
  rows, поэтому повторный переход по вкладкам не показывает лишний loading
  flicker, если данных не меняли;
- process icons теперь берутся только из renderer-safe источников:
  `app_index.list_all` inline `data:image/*` имеет приоритет, stale file-path
  refs из usage-domain отбрасываются;
- добавлен repo-local skill `.agents/skills/visual-verify/`: будущие UI-правки
  должны проверяться не только typecheck'ом, но и Playwright screenshot +
  `view_image`.

Checks: `bun run --cwd platform/desktop typecheck`,
`bun platform/desktop/src/dashboard/store.regression.mjs`, Playwright screenshot checks for
object type colors and usage header alignment.

## 2026-06-01 — Usage playtime precision + process icons (Kosmos Desktop 0.3.7 → 0.3.8)

Usage tracker больше не подменяет playtime игры foreground-временем и не
накручивает часы за просто живой, но свёрнутый процесс.

- `usage_sessions` получил additive-only поле `runtime_ms`; старые строки
  мигрируют из `foreground_ms + idle_ms`, destructive migration нет.
- Tracker ведёт живые process sessions по `(tracked_app_id, pid)` и тикает раз
  в секунду, но runtime засчитывается только пока у процесса есть visible,
  non-minimized top-level window. Это сохраняет Discord/Steam overlay сценарии
  и отсекает “игра свернута в фоне”.
- `foreground_ms` и `idle_ms` остались отдельными диагностическими метриками:
  Dashboard показывает `Суммарное время` из `runtime_ms`, а `Активно` — из
  foreground.
- Единичная Win32/DB ошибка теперь логируется и не убивает tracker task
  навсегда; outer loop перезапускает tracker после fatal failure.
- Usage Dashboard показывает реальные process icons: новые tracked apps пишут
  `tracked_apps.icon_ref` через общий `app_index::icons` PNG cache, а старые
  строки догружают inline icon из `app_index.list_all` по `exe_path`. Если
  иконки нет, слот остаётся пустым, без буквенных placeholder'ов.
- Proof loop: `.agent/tasks/2026-06-01-usage-playtime-precision/`.
- Regression checks: `ark:smoke`, targeted ark-core/backend usage tests,
  platform/desktop/packages typecheck, docs freshness and format checks.

## 2026-06-01 — Dashboard usage data visibility (Kosmos Desktop 0.3.6 → 0.3.7)

Dashboard теперь показывает не только универсальные ARK objects, но и данные
usage tracker'а из отдельных ARK usage tables.

- В sidebar добавлен раздел «Затреканное время».
- Раздел грузит read-only aggregate через `get_usage_analytics`, без raw SQLite
  writes и без расширения object model.
- Таблица показывает process name, суммарное foreground-время, display name,
  количество foreground-отрезков, idle-время, последний запуск и normalized path.
- Колонка «Отрезки» намеренно не называется «Сессии»: tracker режет usage по
  непрерывному `tracked_app_id + PID` foreground interval, поэтому Alt+Tab
  away/back обычно создаёт новый отрезок.
- Таблицы Dashboard используют общий `.kosmos-scroll` из `@kosmos/visuals`;
  scrollbar живёт только в зоне данных под заголовками колонок.
- Root cause зафиксирован в `docs-site/agents/postmortems.md`:
  Dashboard раньше перечислял только `object_types`, а usage tracker хранит
  `tracked_apps` / `usage_sessions` / `usage_events` отдельно.

## 2026-05-30 — Focus widget + extension auto-update (Kosmos Desktop 0.3.5 → 0.3.6)

Release 0.3.6 закрывает два пользовательских хвоста в desktop shell:
компактный pomodoro/focus widget и unattended updates для установленных
расширений.

Focus widget:

- базово показывает только время и название задачи;
- hover/focus раскрывает controls поверх той же зоны, без расширения вправо;
- отдельная drag-handle зона с `GripVertical` отвечает за перемещение;
- position menu получил reset к нижнему центру экрана с отступом 50px;
- progress теперь заполняет саму плашку, без левой border/accent-полосы;
- длинные названия обрезаются ellipsis, не дёргая ширину;
- pause/resume применяют snapshot из backend response сразу, без ожидания
  внешнего event.

Установленные через marketplace/user install расширения теперь обновляются
фоном без открытия Settings и без подтверждения:

- production startup запускает catalog fetch и unattended update;
- каждые 24 часа выполняется force refresh каталога и повторный update pass;
- обновляются только user-installed extensions (`source: "installed"`), repo
  dev-source расширения не трогаются;
- решение об update — strict newer semver, равные/старые/невалидные версии
  пропускаются;
- установка идёт через существующий `installFromUrl()` → `installFromPath()`
  flow, поэтому SHA-256 validation, backup и atomic replace остаются общими.
- открытое Vue-extension окно reload'ится после успешного обновления.

Dev fix: focus-block dynamic chunk снова видит `pingService` и settings-service
helpers через explicit exports из Electron main entry, поэтому `bun run --cwd
shell dev` больше не падает в `pingService is not a function`.

## 2026-05-27 — Native apps experiment + Akasha GPUI reader (historical)

Kosmos получил первый путь для **native extensions**:
приложения остаются частью экосистемы при запуске из Kepler/Kosmos, но могут
существовать как самостоятельные Windows-приложения без привязки к shell'у.
Первым таким экспериментом была Akasha, EPUB-читалка на Rust + GPUI.

С 2026-06-03 это больше не current architecture: Akasha в Kosmos стала
Vue-extension, а старый GPUI-код вынесен в приватный repo `ksanrse/akasha-gpui`.

- `kind: "native"` в extension manifest запускает child process вместо
  Electron `BrowserWindow`.
- Shell передаёт native app аргументы `--kosmos-extension-id` и
  `--kosmos-user-data-dir`; в headless/test mode GUI не spawn'ится.
- Reader UI теперь непрерывный: весь spine рендерится одним scroll surface,
  оглавление открывается верхней кнопкой и скроллится отдельно, parser сохраняет
  heading/list/blockquote + bold/italic spans.
- ARK-backed highlights/notes/RAG не входят в MVP.

## 2026-05-26 — Product rename migration: Kepler → Kosmos (Kosmos Desktop 0.3.3 → 0.3.4)

User-facing desktop app переезжает под один бренд **Kosmos** без ручной
переустановки:

- production Electron artifact: `Kosmos.exe`, installer `Kosmos Setup X.Y.Z.exe`,
  shortcuts `Kosmos`, appId `com.kazui.kosmos`;
- packaged Rust processes читаются в Task Manager понятнее:
  `Kosmos Runtime.exe`, `Kosmos Data Engine.exe`, `Kosmos Helper.exe`,
  `Kosmos System Service.exe`;
- ARK data остаётся в `%APPDATA%\Kosmos`; Electron userData мигрирует из
  `%APPDATA%\Kepler` в `%APPDATA%\Kosmos App` при первом запуске;
- autostart мигрирует с legacy `Kepler` entry на `Kosmos`;
- Windows service CLI понимает legacy `KeplerFocusSvc`, чтобы старые installs
  можно было обслужить без ручного удаления.
- release script пересобирает все packaged Rust exe перед NSIS build, чтобы
  installer не мог забрать stale helper/service binary из `target/release`.

Важно: `kepler:*` IPC, `kepler-backend`, `kepler.lock.json` и package name
`kepler-shell` пока остаются internal/compat names. Слияние `kepler-backend` и
`ark-core-rpc` в один процесс **не входит** в эту миграцию.

## 2026-05-25 — Диктация: state management + UI doводка (Kepler 0.3.1 → 0.3.2)

Bulletproof'инг диктации после второго раунда реального использования.

- **`HotkeyCapture` external mode** для системных shortcut'ов (Win+H, Win+Space) — Settings UI поднимает `begin_hotkey_capture` op'у, hook ловит accelerator ниже системного уровня.
- **`swallow_win_shortcut`** через defer thread + `VK_NONAME` (0xFC) — Start menu не открывается после Win+H. Hook callback возвращается мгновенно (нет race с `LowLevelHooksTimeout`).
- **Defensive cancel в `pillFinished`** + auto-recovery в `start_recording` — backend больше не застревает в Recording state.
- **Auto-repeat дедуп** в hook фикснут — `pressed` flag устанавливается под тем же lock'ом что и check.
- **`Dropdown`** в `@kosmos/visuals`: search автоматом при ≥6 опциях, macOS-style blur + selected fill, fit-content width, white hover border, `cursor: default`.
- **Settings → Диктация:** `RadioGroup` → `Dropdown` для «Режим триггера» и «Вставка».

Известная проблема: **Win+Ctrl+V audio picker иногда пробивается при Win+H** — в техдолге (`docs-site/concepts/dictation.md` § Known issues). Workaround: отпускать Win раньше H.

## 2026-05-25 — Диктация polish (Kepler 0.3.0 → 0.3.1)

Полировка диктации после первого реального использования. 4 серьёзных бага + UX-фиксы.

- **Capture системных hotkey'ев** (Win+H, Win+Space) через новый system-level capture mode в hook'е. Toggle mode тоже идёт через hook → Electron `globalShortcut` больше не используется.
- **Persistent audio stream** с 30s idle keep-alive — 0ms latency на серии диктовок.
- **Idle warmup pill window** через 3s после старта Kepler.
- **Visual polish**: pill 240×72 снизу 100px от низа, чёрный glossy без drop-shadow, только waveform внутри.

Post-mortems (см. [`docs-site/concepts/dictation.md`](docs-site/concepts/dictation.md) § Post-mortems):

1. **TryFromIntError в inject** — `enigo::Key::Unicode('v')` ломался; перешли на нативный `windows::Win32::SendInput`. `enigo` dep удалён.
2. **Pill race condition** — первый `start` терялся до Vue mount; добавлен `pillReady` Promise.
3. **Модификаторы не ловились в capture** — hook intercept'ил Win-down → GetAsyncKeyState возвращал false; modifier'ы теперь пропускаются.
4. **Start menu после Win+H intercept** — Win-up без других клавиш триггерит Start; AHK/PowerToys приём с dummy `SendInput VK_RESERVED` чтобы Windows считала Win использованной как modifier.

## 2026-05-25 — Диктация (STT) Phase 1 + 1.5 (Kepler 0.2.10 → 0.3.0)

Голосовой ввод по образцу Raycast Dictation. Hotkey → pill снизу экрана → Groq Cloud (whisper-large-v3) → авто-вставка в активное окно через нативный Win32 `SendInput`.

### Что есть в 0.3.0

- **Backend модуль** `platform/runtime/src/dictation/`: host (state machine + broadcast events), Groq client с anti-hallucination фильтрацией сегментов (`no_speech_prob > 0.6`, `avg_logprob < -1.0`) + `temperature=0` + захардкоженный prompt, inject через `windows::Win32::SendInput` (`Ctrl+V`), Windows Credential Manager для API-ключа, DoH/SOCKS proxy для AI HTTP, low-level keyboard hook для push-to-talk, persistent stats (WPM / Time Saved / Total Words).
- **Pill window** — frameless, alwaysOnTop, `focusable: false`, 200×56 в окне 240×72, снизу экрана 100px от низа. Glossy чёрный с waveform внутри. Audio capture в renderer через Web Audio API (16kHz mono PCM → WAV → base64).
- **Settings**:
  - **Безопасность** — DNS-резолвер (System / Cloudflare DoH / Google DoH / Custom) + HTTP/SOCKS proxy + test connectivity.
  - **Секреты** — Groq API key (keyring).
  - **Диктация** — статистика-карточки (Raycast-style), выбор микрофона, язык (23 варианта), hotkey, trigger mode, inject mode, провайдер.
- **`@kosmos/visuals`** — 6 новых компонентов: `Button`, `TextInput`, `Textarea`, `RadioGroup`, `HotkeyCapture`, и settings primitives.

Proof loop: `.agent/tasks/2026-05-24-dictation/spec.md`. Docs: `docs-site/concepts/dictation.md`. ADR: `docs-site/reference/decisions.md` § 2026-05-24.

## 2026-05-23 — Settings sidebar + command visibility (Kepler 0.2.7 → 0.2.8)

Settings window переработан с нуля: теперь это sidebar-first layout с поиском по разделам.

### Что изменилось

- **Sidebar navigation** — левая панель 229px с поиском, двумя группами («Общие», «Расширенные») и per-страничными иконками.
- **Новые страницы** — О приложении, Дебаг, Заметки, Задачи, Времяметр, Игры, Поиск файлов, Фокус, Расширения — каждая с `SettingsAdvancedIntro` header'ом.
- **Перегруппировка** — «Версия Kepler» → «О приложении»; Developer mode + Usage tracker → «Дебаг»; File search toggle → «Поиск файлов».
- **Tray icon toggle** — новая настройка «Показывать в трее» в Общих (persistent, live — трей пересоздаётся без перезапуска).
- **Command visibility** — в разделах Заметки / Задачи / Времяметр / Игры можно отключить конкретные команды из launcher'а (хранится в localStorage, лаунчер реагирует через storage event).
- **Window chrome** — `backgroundMaterial` сменён с `mica` на `acrylic`; titleBar hidden + native overlay (36px).
- **`@kosmos/visuals` — 5 новых компонентов**: `SettingsSidebar`, `SettingsSidebarButton`, `SettingsSearchInput`, `SettingsList`, `SettingsAdvancedIntro` + settings CSS-переменные и токены цветов.

Proof loops: `.agent/tasks/2026-05-23-settings-sidebar-visuals/`, `.agent/tasks/2026-05-23-settings-advanced-layout/`, `.agent/tasks/2026-05-23-settings-file-search-page/`.

## 2026-05-23 — Fast File Search через Windows Service (Kepler 0.2.6 → 0.2.7)

File Search v1 доведён до fast-path архитектуры на Windows:

- `platform/runtime/src/file_index/scanner/ntfs.rs` теперь сначала
  обращается к `KeplerFocusSvc` через named pipe `\\.\pipe\kepler-focus-svc`
  и просит `ntfs_scan`.
- `platform/native-services/kepler-focus-svc/src/ntfs_scan.rs` читает MFT через
  `ntfs-reader` под LocalSystem token'ом service'а. Kepler UI и
  `kepler-backend.exe` остаются user-level процессами; UAC нужен только при
  установке service'а.
- Если service не установлен / не отвечает / старой версии, backend логирует
  `ntfs service scan unavailable`, пробует локальный `ntfs-reader`, затем
  обычный walk fallback. Это сохраняет совместимость с машинами без service.
- Pipe protocol `kepler-focus-svc` расширен backward-compatible операцией
  `ntfs_scan { root, exclude_noisy } -> { files: [{ path, name, mtime }] }`.
- Исправлен stop-path Windows Service: service больше не зависает на
  `pipe_thread.join()` при SCM stop, если pipe thread стоит в `ConnectNamedPipe`.

Proof loops:

- `.agent/tasks/2026-05-23-file-search-ntfs-reader/spec.md`
- `.agent/tasks/2026-05-23-file-search-ntfs-service/spec.md`

## 2026-05-22 — File Search v1

Kepler launcher теперь ищет **файлы по имени и пути** через отдельный
host-local индекс в `kepler-backend`:

- `platform/runtime/src/file_index/` хранит индекс в `file-index.db`
  рядом с instance data, не в ARK и не в sync.
- V1 сканирует файлы на локальных fixed drives; на NTFS drive roots backend
  сначала пробует MFT/USN fast scan и при недоступности прозрачно падает назад
  на обычный non-elevated обход. Folder results и content search пока не входят
  в скоуп.
- Шумные папки (`node_modules`, `.git`, `dist`, `target`, temp/build cache)
  исключаются по умолчанию. Toggle в Settings → Общие сохраняет режим в
  `file-index.db` и сразу запускает reindex.
- Launcher подмешивает file hits только когда введён query, показывает pending
  state пока backend ищет файлы и открывает файл через backend
  `file_index.open`.

Proof loop: `.agent/tasks/2026-05-22-file-search-v1/spec.md`.

## 2026-05-22 — App Launcher v1 + state restore (Kepler 0.2.4 → 0.2.5)

Большая новая фича — **запуск установленных приложений** из поисковика
лаунчера (`Alt+Space`). Раньше поисковик находил только команды
расширений; теперь — все Win32-программы (Start Menu) и UWP / Microsoft
Store apps, в общем списке с командами.

### Что под капотом

- `platform/runtime/src/app_index/` — новый Rust-модуль:
  trait `AppSource` + per-platform impl (Windows: Start Menu + UWP),
  отдельный SQLite `app-index.db` рядом с `ark.db` (host-specific,
  НЕ в ARK — см. write-boundary), in-memory cache, icon extractor
  (Win32 ExtractIconExW + UWP Package.GetLogo с trim transparent
  padding), WS namespace `app_index.{list_all,search,launch,rescan}`.
- Иконки приходят renderer'у как **inline base64 data URL** (renderer
  без file:// access). 65 apps × ~30KB = ~2MB JSON — приемлемо для
  local IPC.
- Cross-platform-ready: macOS / Linux добавятся как новые `AppSource`
  impl без переписывания общей логики.

### Discord/Slack/Teams icon fix

Squirrel-installer apps кладут target=Update.exe (без embedded icons),
а реальный icon location прописан в `.lnk`. Старый extractor доставал
иконку только из target → placeholder. Теперь Start Menu source
вызывает icons.rs eagerly с доступом к .lnk path, стратегия:

1. `.lnk::icon_location()` → 2) target exe.

### State restore с TTL

Launcher запоминает `{ query, selectedIndex, scrollTop, savedAt }`
в localStorage (debounce 200ms). При следующем открытии — если прошло
меньше TTL (default 5 мин), восстанавливается. После invoke — стирается.
Setting в Settings → Общие.

См. `.agent/tasks/2026-05-22-app-launcher/spec.md` и `docs-site/concepts/app-index.md`.

## Текущие версии

| Артефакт                                            | Версия                                                                           |
| --------------------------------------------------- | -------------------------------------------------------------------------------- |
| Kosmos Desktop (`platform/desktop/package.json`)    | **0.4.8**                                                                        |
| Akasha extension (`incubator/akasha/manifest.json`) | **0.1.2** (Vue EPUB reader; EPUB parser guardrails)                              |
| Eden extension (`products/eden/manifest.json`)      | **0.2.1** (sidebar/object layout polish + дневник button + accent state cleanup) |
| Delphi extension                                    | **0.1.7** (live ARK sync + «Когда-нибудь» + layout-agnostic Ctrl)                |
| Arrancador extension                                | **0.1.4**                                                                        |
| Dashboard                                           | встроен в shell (не extension)                                                   |

1. Floating focus widget переставал тикать (renderer push'ил state, renderer dead → no push'ей; main process autonomous tick не обновлял phaseEndsAtMs на phase boundary).
2. `time_entry_obj` оставались `endedAt: null` навсегда (close/create логика жила в `usePomodoroSession` renderer'е).
3. Native notifications зависели от renderer-life'а.

**Решение**:

- **`keepAliveInBackground: true`** новое поле в `ExtensionManifest`. `platform/desktop/electron/extension-host.ts` intercept'ит `close` event для таких extensions → `win.hide()` вместо destroy. Renderer переживает X, все side-effects продолжают работать. Окно реально destroy'ится только на `app.before-quit`.
- **`backgroundThrottling: false`** теперь для всех extension `webPreferences` — даже hidden / minimized таймеры не throttle'ятся Chromium'ом.
- **Drag fix виджета** — `focusable: false` на BrowserWindow ломал `-webkit-app-region: drag` (Win32 не отправляет WM_NCLBUTTONDOWN не-фокусабельному окну). Поменяли на `focusable: true`; `showInactive()` всё ещё обеспечивает «не воровать фокус при появлении».

### @kosmos/visuals → `IconButton` primitive

Выделен ghost icon button (`size`, `tone: default | destructive`, `draggable`). Заменяет ad-hoc `.iconbtn` / `.ctl-btn` / `.close-btn` CSS, которые тиражировались по shell / extensions. Extension windows используют native controls через Electron `titleBarOverlay`; renderer-компонент `WindowControls` удалён из `@kosmos/visuals`.

### Eden 0.1.10 → 0.1.11

- **Auto-scroll при drag-select** — Anytype-style rubber-band selection теперь скроллит editor когда курсор у верх/низ края `.kosmos-scroll` контейнера. rAF-loop активен пока drag активен: двигает scrollTop, anchor'ит startY к документу (не к viewport), пересобирает cache блоков, пересчитывает selection даже при неподвижной мыши. Edge zone 48px, max 16px/frame.
- **Wikilink визуально на Eden accent** (оранжевый) вместо общего синего — линки в заметках смотрятся как часть Eden theme'ы, а не системные.

## 2026-05-20 — Eden Pattern B + Anytype block selection (Kepler 0.2.0 → 0.2.1)

## 2026-05-20 — Eden Pattern B + Anytype block selection (Kepler 0.2.0 → 0.2.1)

Большая итерация по Eden и cross-extension инфраструктуре.

### Eden 0.1.0 → 0.1.8: TipTap TaskList → Pattern B (task = object)

- **TipTap TaskList → TaskRef NodeView** (Pattern B refactor): task в заметке = ссылка
  на `task_obj` в ARK, не дубликат текста. Vue NodeView подписан на ARK
  `object_upserted`/`object_deleted` events, live обновляется когда Delphi меняет
  task. Title редактируется inline в input. Single click — открыть, Backspace
  на пустом — soft-delete.
- **Linear-style 5 статусов задач**: triage (default для новых) / backlog / todo /
  done / canceled. ПКМ-меню для выбора. Custom SVG icons на TaskStatusIcon.vue.
- **Anytype-style rubber-band block selection**: composable `useBlockSelection`,
  document-level mouse handlers с 20px threshold + block-crossing activation
  (drag в одной строке = native text select, drag через блоки = rubber-band).
  Visual через PM Decoration API (не direct DOM mutation — стирается на PM
  re-render). Esc clears, Delete удаляет блоки (для taskRef + soft-delete task_obj).
- **`/задача` и `- [ ]` markdown shortcut** создают TaskRef.
- **Persist state**: zen mode + last visited entry в localStorage. Reload
  возвращает в ту же заметку в том же режиме.
- **Char counter в zen mode**: IBM Plex Mono, fully transparent footer +
  border-top при scroll, padding-bottom редактору чтобы text упирался выше.
- **Brand orange каретка** для всех inputs/contenteditable.
- **CSS dedupe**: убраны 4 копии `.ProseMirror > * + *` правила, merge resize-handle
  3 копий, reset `<p>` margin. Unified 2px inter-block gap.

### Foundation (ark-core + shell)

- **`ark-core` local `entity_changed` events**: до 2026-05-20 эмитились только
  на sync-incoming изменения; теперь и на локальные `upsert_object` /
  `delete_object`. Cross-app live reactivity (Eden TaskRef ↔ Delphi list) теперь
  работает без specific protocols.
- **`ws_server` forward** ark-core events клиентам — был latent gap,
  forward'ились только command_bus + pomodoro events.
- **Shell: focus existing extension window** на повторный invoke. Включая
  minimized/hidden cases через AOT-toggle workaround для Win32
  `SetForegroundWindow` restriction. Применимо ко всем extension'ам.

### Delphi 0.1.1 → 0.1.3

- **Live sync через ARK events**: подписка на `object_upserted` для `task_obj`
  → refresh task list. Eden создаёт/меняет задачу — Delphi мгновенно видит.
- **Вкладка «Когда-нибудь»** в sidebar — задачи с `propsJson.status === "backlog"`
  (плюс legacy `isSomeday`). Иконка Archive.

### Visuals

- Новый primitive `Checkbox` — outline + inner filled square (Delphi
  `.check-box` parity). Brand accent через CSS var override (Eden — orange).

### Tooling

- Drive-by fix `scripts/ark-smoke.mjs` — Windows quote bug при `shell:true` +
  пробелы в path к `node.exe`.

## 2026-05-18 — performance sweep + UX полировка (Kepler 0.1.10 → 0.1.16)

Серия release'ов за день:

- **0.1.10** — Mica backdrop + electronLanguages shrink (−45 MB disk) + CSS contain + TS incremental
- **0.1.11** — Export bug fix (unwrap `{converters: [...]}`) + extensions catalog populated
- **0.1.12** — Marketplace UI (Settings → Расширения → catalog с кнопкой Установить) + Export tab скрыт (техдолг)
- **0.1.13** — Launcher commands filtered by installed extensions (Eden / Delphi / etc не показываются если не установлен)
- **0.1.14** — Toggle цвет = акцент (был зелёный) + streamer mode (Chromium occlusion off, shell-wide)
- **0.1.15** — **Focus widget** — Spotify-mini-player-style плавающий always-on-top окно для активной pomodoro

См.:

- `docs-site/concepts/system-requirements.md` — что нужно для запуска и сборки (canonical).
- `docs-site/concepts/performance-experiments.md` — реальные baseline/after measurements (правило: нет цифр → `(не записал)`, **никогда** не выдумывать).
- `.agent/tasks/2026-05-18-*/evidence.md` — proof loops с calibration.
- `packages/visuals/STORYBOOK.md` — Storybook contributor guide.
- `.agent/tasks/2026-05-18-focus-mode-digital-cave-spec/spec.md` — roadmap для digital-cave merger.

## 🛡 Production hardening (2026-05-18)

Подготовлен safety net для distributed local-first продукта с real users.
Proof loop: `.agent/tasks/2026-05-18-pre-in-process-hardening/`. Детали:
[docs-site/concepts/db-resilience.md](./docs-site/concepts/db-resilience.md).

- **DB backup** на старте backend — раз в 24h, rotation 7. `<data_dir>/backups/`.
- **`PRAGMA integrity_check`** в `init_schema` — fail loud на corruption.
- **Backend auto-respawn supervisor** в Electron main — exponential backoff (1s→5s→30s→1m→2m), error dialog после 5 streak'ов.
- **Crash reporter** — Rust panic_hook → `<data_dir>/crashes/panic-*.log` + Electron `crashReporter.start()`. Settings UI секция "Отчёты об ошибках".
- **Mutex poison recovery** — SqliteStorageBackend методы.
- **Property-based tests** — `cargo test --test proptest_invariants` (3 properties × 64 cases).

In-process facade (subprocess removal) — отложено: subprocess нужен пока как crash isolation layer. Hardening это **prerequisite**, не subprocess removal сам.

## 🟡 Хранимый техдолг (2026-05-18)

- **Export tab** скрыт в Settings (whitescreen на production 0.1.11). См. `docs-site/agents/manual-tests-pending.md` → tech debt entry. Возврат после DevTools debug.
- **`vue-router` mock в Storybook preview** — Sidebar/SidebarButton stories skipped (зависят от RouterLink).
- **Light theme** — TODO в `packages/visuals/.storybook/preview.ts` (theme toolbar item закомментирован).

Итог архитектурного pivot'а от standalone Electron-апок к Kepler-host архитектуре с Vue extensions. **Все 5 апок мигрированы** (Eden — Phase 6.0 + 6.0.A, 2026-05-17). После 2026-05-15: концепция spaces убрана (single DB per user), Dashboard встроен в shell, e2e Playwright suite зелёный. После 2026-05-17 (Phase 6.0.A): standalone `apps/eden/ts/` удалён, Hevy/code-tools убраны из Eden (Hevy → Olympia позже).

## Архитектура

```
Kepler.exe (Electron host)
  ├─ launcher window (Ctrl+Shift+K, fixed 720×460, acrylic)
  ├─ settings window (tray menu)
  ├─ Dashboard window (embedded shell view — read-only ARK browser)
  ├─ extension-host
  │   ├─ products/delphi/      (Vue bundle inside Kepler)
  │   └─ incubator/arrancador/  (Vue bundle inside Kepler)
  └─ spawn kepler-backend.exe (Rust, headless)
      ├─ ark-core-rpc child (SQLite WAL, FTS5)
      ├─ WS server 127.0.0.1:<port>
      ├─ command bus (registry + invoke broadcast)
      └─ LAN sync (centralized — один node на машину)

Eden — Vue extension в `products/eden/` (Phase 6.0 / 6.0.A, 2026-05-17). Standalone `apps/eden/ts/` удалён полностью.
```

## Naming convention (после brand swap)

| Слой                                   | Имя                  |
| -------------------------------------- | -------------------- |
| Ecosystem (monorepo, ARK SDK, AppData) | **Kosmos**           |
| Launcher app (Electron host)           | **Kepler**           |
| TS package SDK                         | `@kosmos/ark`        |
| Visuals (CSS tokens + Vue components)  | `@kosmos/visuals`    |
| Backend binary                         | `kepler-backend.exe` |

## Изоляция инстансов (slot system, 2026-05-20)

Каждой запущенной копии Kepler присваивается **slot**, на основе которого
derive'ятся Electron userData, ARK dataDir, productName, hotkey, autoupdater,
autorun. Цель — installed prod Kepler работает **одновременно** с dev-сессиями
(и multi-agent worktree разработкой).

| slot             | trigger                                                                      | Electron userData             | ARK dataDir                 | hotkey      | autoupdater |
| ---------------- | ---------------------------------------------------------------------------- | ----------------------------- | --------------------------- | ----------- | ----------- |
| `prod` (default) | installed `Kepler.exe`                                                       | `%APPDATA%\Kepler\`           | `%APPDATA%\Kosmos\`         | `Alt+Space` | on          |
| `dev`            | `VITE_DEV_SERVER_URL` (`bun run --cwd platform/desktop dev`)                 | `%APPDATA%\Kepler-dev\`       | `%APPDATA%\Kosmos-dev\`     | `` Alt+` `` | off         |
| `dev-<x>`        | `KEPLER_INSTANCE=dev-<x>` (per-worktree `.env.local` in `platform/desktop/`) | `%APPDATA%\Kepler-dev-<x>\`   | `%APPDATA%\Kosmos-dev-<x>\` | disabled    | off         |
| `test-<x>`       | Playwright (`KOSMOS_DATA_DIR` set)                                           | `<KOSMOS_DATA_DIR>/userdata/` | `KOSMOS_DATA_DIR`           | disabled    | off         |

Single source of truth: `platform/desktop/electron/instance.ts::resolveInstance()`.
`applyInstanceToApp()` вызывается в самом верху `main.ts` — до
`requestSingleInstanceLock`, чтобы lock scope'ился по новому userData.

Полная документация — [Instance slots](docs-site/concepts/instances.md).

## ✅ Сделано

### Phase 0 — Backend extraction

- `platform/runtime/` (lib + bin) — Rust headless service. Extracted из старого legacy Rust launcher.
- 41 → 46 unit tests passing (5 новых для command_bus).

### Brand swap (Kepler ↔ Kosmos)

- 836 файлов pre-swap → post-swap. Старые apps `Kepler` → ecosystem `Kosmos`. Старый launcher `Kosmos` → новый `Kepler`.
- User data migration script: `scripts/migrate-kepler-to-kosmos.ps1` + smoke test (16 assertions pass).
- ADR: `docs/MIGRATION-2026-05-14-brand-swap.md`.

### Phase 1 — Electron Kepler shell

- `platform/desktop/` — Electron + Vue 3.6 + electron-vite + TypeScript.
- Frameless launcher 720×460, acrylic Mica на Win11, globalShortcut Ctrl+Shift+K.
- Tray icon + menu (Открыть / Настройки / Выход).
- Spawn `kepler-backend.exe` child + auto-connect через `ensureKeplerRunning`.
- Launcher fixed position: center horizontally, `15%` от высоты primary workArea сверху.
- DevTools auto-open detached в dev mode.

### Phase 2 — Command bus full stack

- `platform/runtime/src/command_bus.rs` — Rust WS-протокол.
  - Operations: `commands.register / unregister / list / invoke`.
  - Events: `command_invoked` / `commands_changed` broadcast.
  - Auto-unregister на WS disconnect.
- `@kosmos/ark` SDK: `client.commands.{register,unregister,list,invoke,onInvoked,onChanged}` namespace + types (`CommandManifest`, `CommandInvokedEvent`).

### Phase 3 — Real action handlers

- **Delphi**: `task:create` → QuickEntry, `task:today` → router.push '/today'.
- **Eden**: `note:create` → `createNewEntry`, `note:search` → `openSearch`.
- **Kepler settings window** — separate BrowserWindow с автозапуском HKCU toggle + backend status + version.
- **Extension loader PoC** — foundation для Phase 4.

### Phase 4 — Apps как Vue extensions

| App            | Build                              | Что работает                                                                                                                                             | Что осталось                                                                       |
| -------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| **Dashboard**  | 83 KB JS / 17 KB CSS               | Read-only analytics: Overview + Sessions pages через `kepler.ark.request("get_usage_analytics")` + focus/visibility auto-refresh                         | Choose/reset DB — host-managed; vue-router history dropped                         |
| **Delphi**     | 3485 modules, 32 KB CSS с Tailwind | Vue Router (memory history), 5 pages, `electron-api-shim.ts` устанавливает `window.electronAPI` поверх `kepler.ark.request`. Task CRUD работает          | LAN sync / P2P / space management — graceful no-op. Tailwind остался (см. Phase 9) |
| **Arrancador** | 108 KB JS / 19 KB CSS, 57 modules  | 7 страниц (Library + 6 stubs/read-only): Library, Catalogue (stub), Scan, Sqoba (stub), Statistics (JS aggregation), Settings (localStorage), GameDetail | Native scanner spawn, game launch, RAWG metadata fetch — Phase 5+                  |

### Phase 5 — Extension developer mode (Raycast-style)

- `KEPLER_DEV=1` env var → extensions грузятся с `http://localhost:<port>/` вместо `dist/index.html`. HMR работает.
- F12 toggles DevTools на любом extension window.
- Settings → Developer Mode toggle (persist в `kepler-shell-settings.json`).
- `bun run dev:extensions` orchestrator + `bun run dev:kepler` paired workflow.

### Phase 8 — Production packaging

- `electron-builder` NSIS config: `platform/desktop/package.json` `build` section.
- `extraResources`: `kepler-backend.exe` + `ark-core-rpc.exe` + `extensions/<id>/dist+manifest+icon` + tray icon.
- `afterPack.cjs` hook — PNG → ICO + rcedit embed metadata в `Kepler.exe`.
- `bun run --cwd platform/desktop build` → `platform/desktop/release/Kosmos Setup X.Y.Z.exe`.
- Install path: `%LOCALAPPDATA%\Programs\Kepler\` (per-user oneClick).

### Inter-app communication

- `@kosmos/ark` cosmos-mode — apps подключаются к single `kepler-backend` WS, делят `ark-core-rpc`. Один sync node на машину (-3 ark-core-rpc).
- Command bus: launcher → backend → apps event broadcast → handler execute.
- Selected space DB resolution — `kepler-shell` передаёт `KOSMOS_DB_PATH` в backend (читает `selected-space.json`).

### RAM benchmark

| Метрика       | Baseline (4 standalone) | Kepler + 4 extensions | Diff               |
| ------------- | ----------------------- | --------------------- | ------------------ |
| Working Set   | 1092 MB                 | 968 MB                | **−124 MB / −11%** |
| Private Bytes | 683 MB                  | 474 MB                | **−209 MB / −31%** |
| Processes     | 15                      | 11                    | −4                 |

Caveats: оба measurements в dev mode (DevTools overhead +~160 MB). Real prod без DevTools — save аналогичный.

### Documentation

- `docs-site/concepts/`: architecture, command-bus, extension-host, extension-dev-mode, ram-benchmarks, sync, ark-objects, write-boundary, test-isolation.
- `docs-site/packages/`: kosmos-ark (с Commands API), kosmos-visuals, ark-core.
- `docs-site/reference/`: commands, rules, decisions, smoke-matrix.
- `docs-site/agents/`: index, checklists, forbidden, docs-maintenance.

### Phase 10 — Post-migration stabilization (2026-05-15)

Серия багов после Phase 4 (Vue extensions). См. `.agent/tasks/2026-05-15-post-migration-fixes/{spec,evidence}.md` для proof loop.

- **Drop spaces concept** (single DB per user) — welcome screen и space-picker убраны. Dashboard сразу открывается на список объектов. Делphi shim emits stub `KEPLERDEFAULT` чтобы не показывать SpaceSetup modal.
- **Master bug — `missing field 'id'`** (`platform/runtime/src/ws_server.rs`): backend strip'ил `id` из params как envelope id, ломал `get_object`/`delete_object`/`upsert_object` с top-level id. Fix: чтит `_req_id` для envelope, оставляет `id` нетронутым. Это разблокировало все extension CRUD.
- **Extension ARK bridge ready-gate** — `kepler:extension:ark:request` ждёт arkClient connect (15s timeout) вместо мгновенного throw. См. commit fdfc8d4.
- **Делphi task persistence** — lazy ensure `task_obj` object_type перед первым upsert. FK constraint failed → silent swallow в `.catch()` → задача жила in-memory. Fix: registered + warn вместо silent.
- **Dev mode data isolation** — `keplerDataDir()` returns `Kosmos-dev` в dev, `Kosmos` в prod, `KOSMOS_DATA_DIR` override в test. shell+backend смотрят на один dir.

### Phase 6.0 / 6.0.A — Eden как extension (2026-05-17)

См. `.agent/tasks/2026-05-17-eden-extension/spec.md` и `.agent/tasks/2026-05-17-eden-cleanup-and-hardening/spec.md`.

**Phase 6.0 — Scaffold + ARK note CRUD**:

- `products/eden/` создан как Vue extension (manifest, package, vite config, src/).
- `kepler-api-shim` (renderer-side bridge поверх `window.kepler.ark.request`) — emulates `window.api` так, что Eden codebase почти не правился.
- Note CRUD / folders / typed-notes / search — все ARK операции через shim.
- Команды `eden:note:create` / `eden:note:search` зарегистрированы в command bus.

**Phase 6.0.A — Cleanup + hardening**:

- Hevy полностью удалён (UI + API + ConnectedAppsSettings.vue + lib/hevy.ts). Замена — Olympia.
- Code lint/format удалён полностью (Editor.vue вызовы, settings panel, shim methods, vite-env types).
- Trash UI реализован поверх ARK soft-delete (`deletedAt != null` фильтр; restore через `upsert_object` с `deletedAt: null`).
- Bundle codesplit: lazy `Editor.vue` через `defineAsyncComponent` → main bundle **353KB** (gzip 112KB), editor chunk 1.36MB lazy.
- Standalone `apps/eden/ts/` (вместе с Heart Rust + main process + preload) удалён полностью.
- Workspace + tooling cleanup: `package.json`, `Cargo.toml`, `lefthook.yml`, scripts/\*.mjs.

### Arrancador full completion (2026-05-18)

См. `.agent/tasks/2026-05-18-arrancador-full-completion/spec.md`.

- Backend `platform/runtime/src/arrancador/` — 5 modules (scanner, launcher, rawg, sqoba, config), 32 unit tests + 3 integration tests с synthetic Steam library (Dota 2 / Cairn / Outlast).
- Scanner: Steam VDF/ACF custom parser (без новых deps) + Epic JSON manifests. GOG skip.
- Launcher: `steam://rungameid/<id>` через `cmd /c start` + прямой exe spawn для Epic/manual.
- RAWG client: search + get_details + apply (merge в game_obj.propsJson), httpmock тесты.
- SQOBA: discover save paths heuristics, zip с `_sqoba_meta.json`, restore с path traversal protection, rotation keep N=10.
- WS namespace `arrancador.*` (scan/launch/rawg._/sqoba._/config.\*).
- UI: все 4 stub'нутые страницы оживлены (Library launch button, Scan кнопка + history, Catalogue RAWG search+apply Modal, Sqoba per-game backup/restore, Settings RAWG key).
- Preload bridge `window.kepler.arrancador.*`.
- cargo test 110/110, typecheck/build/ark-guard зелёные.

### Phase 7 — Universal per-type data export (2026-05-18)

См. `.agent/tasks/2026-05-18-phase-7-universal-export/spec.md` и `docs-site/concepts/data-export.md`.

- Rust `Converter` trait + registry в `platform/runtime/src/export/` + WS endpoints `export.list` / `export.run`.
- 6 первых конвертеров: `note_md` (TipTap → markdown с YAML frontmatter), `task_md`, `task_csv`, `time_entry_csv`, `tag_json`, `game_json`.
- Shell UI: новый таб «Экспорт» в `platform/desktop/src/views/SettingsView.vue` — per-converter карта, native directory picker, история экспортов (10 шт в localStorage).
- 17 unit tests для converters; cargo test 75/75 зелёный.
- ARK guard:writes остался clean (export — read-only через `list_objects_by_type`).
- Eden export-to-markdown заменён этим универсальным механизмом.

### Lock-file test isolation (2026-05-17)

env-флаг `KOSMOS_LOCK_PERMISSIONS_DISABLED=1` в `platform/runtime/src/lock_file.rs` пропускает icacls/chmod ACL-хардинг в test mode. `tests/e2e/helpers/launch.ts` выставляет автоматически. Решает проблему stale ACL lock-файлов при смене Windows account'а.

### Visuals unification (2026-05-18)

### Phase 10 — Playwright e2e infrastructure

`tests/e2e/` — 13 specs, single worker, isolated DB per spec под `tests/.e2e/<slug>/`. Helper `tests/e2e/helpers/launch.ts` refuses paths inside `%APPDATA%`. Backend читает `KOSMOS_DATA_DIR` env override.

```powershell
bun run test:e2e            # full suite (~1.5min, 13/13 PASS)
bun run test:e2e:headed     # visible Electron
bunx playwright test --list # parse-check
```

### Migration scripts

- `scripts/migrate-kepler-to-kosmos.ps1` — user data `%APPDATA%\Kepler` → `%APPDATA%\Kosmos` (atomic Move-Item + lock-файл renames + HKCU Run update).
- `scripts/migrate-kepler-to-kosmos-smoke.ps1` — isolated smoke test (16/16 assertions pass).
- `scripts/check-swap-completeness.ps1` — grep audit forbidden token patterns.
- `scripts/measure-kepler-ram.ps1` — baseline / kepler / `-Compare` modes для RAM benchmarks.
- `scripts/fix-mojibake.mjs` — UTF-8 recovery после PowerShell encoding bugs.
- `scripts/merge-swap.mjs` — token swap helper после `git checkout --theirs` merge conflicts.
- `platform/desktop/scripts/dev-extensions.mjs` — orchestrator для Vite dev servers per extension.

## ⏳ Не сделано / отложено

### Phase 7 — Adaptive lifecycle (optional)

LRU eviction, RAM budget management, lazy extension load/unload. Имеет смысл только если open extensions >> память бюджет — для 4 текущих не критично.

### Phase 8 — Retire legacy

После production smoke testing:

- Старый Rust GPUI launcher — удалить, если ещё всплывут остатки после migration cleanup.
- Eden — оставить пока не сделано Phase 6.
- Auto-update mechanism (`electron-updater`) — **подключён** (Phase 8b, 2026-05-16): `platform/desktop/electron/autoupdater-host.ts` (state machine: idle/checking/available/downloading/downloaded/error), Raycast-style banner в Settings, launcher-команда `kepler:check-updates`, кнопка «Проверить обновления» в General. Distribution через `yoso-industries/kepler-releases`. См. [Distribution](docs-site/concepts/distribution.md#kepler-launcher-autoupdater).

### Phase 9 — Delphi UI на plain CSS (open question)

Delphi extension использует **Tailwind v4** (наследие legacy standalone Delphi). Все остальные extension'ы + Kepler launcher / settings — на **plain scoped CSS + `@kosmos/visuals` CSS variables**.

Что нужно для Phase 9 (open question):

- ~30 .vue файлов в `products/delphi/src/` — удалить Tailwind utility classes из templates.
- Переписать стили в `<style scoped>` с CSS vars из `@kosmos/visuals`.
- Удалить `@import "tailwindcss"` + `@source` из `products/delphi/src/global.css`.
- Удалить `@tailwindcss/vite` plugin из vite configs.
- Удалить tailwind deps.

Скоуп — несколько часов сфокусированной работы. Откладывается до момента когда Delphi UI стабилизируется.

### Phase 10+ — Dynamic extension store

- Manifest URL discovery (extension может объявить `updateUrl`).
- Download + install third-party extensions без переустановки Kepler.
- Permission model (которые ARK operations extension может вызывать).
- Extension marketplace / signing — long-term.

Сейчас все 4 extensions ship'ятся вместе с Kepler как extra resources.

### Что ещё в TODO

- **Arrancador**: native scanner integration (либо migrate в `kepler-backend`, либо отдельный sidecar spawned by kepler-shell).
- **Dashboard**: aggregation в renderer'е (group/sum по usage_sessions).
- **Arrancador**: RAWG metadata fetch (HTTP proxy через kepler-backend).
- **Game launch** (Arrancador): нужен `kepler.window.launch(exe)` API в shell.
- **Manual smoke test infrastructure**: Playwright skeleton есть, реальные тесты для extensions не написаны.
- **Real RAM benchmark в prod mode** (без auto DevTools).

## Команды

### Dev workflow (Raycast-style)

```cmd
:: Terminal 1: Vite dev servers per extension с HMR
bun run --cwd platform/desktop dev:extensions
::   → dashboard:  http://localhost:5180/
::   → delphi:     http://localhost:5182/
::   → arrancador: http://localhost:5183/

:: Terminal 2: Kepler shell + main process
bun run --cwd platform/desktop dev:kepler

:: или для проверки prod build:
bun run --cwd platform/desktop dev
```

### Production build

```cmd
cd apps\kepler-shell
bun install
bun run build
:: → release/Kepler Setup 0.1.6.exe (3-5 min cold)
```

### Verify

```cmd
:: Backend Rust + 46 unit tests
cargo build --manifest-path platform/runtime/Cargo.toml --bin kepler-backend
cargo test --manifest-path platform/runtime/Cargo.toml --lib

:: TypeScript
bun run --cwd core/ark/packages/ark typecheck
bun run --cwd platform/desktop typecheck

:: Extensions build
bun run --cwd platform/desktop build:extensions

:: RAM benchmark
pwsh scripts/measure-kepler-ram.ps1 -Mode baseline
pwsh scripts/measure-kepler-ram.ps1 -Mode kepler
pwsh scripts/measure-kepler-ram.ps1 -Compare
```

### Migration (если ещё не сделан)

```cmd
pwsh scripts/migrate-kepler-to-kosmos.ps1 -WhatIf   :: preview
pwsh scripts/migrate-kepler-to-kosmos.ps1           :: execute
```

## История ветки kosmos/phase-1-scaffold

```
402f931 polish (Tailwind / Delphi shim / Arrancador pages) + NSIS packaging
6927cd5 mtime-based icon cache invalidation
6f09086 crash on close + app icons + status dot + selected-space DB
54fc0fe extension dev mode (Raycast-style HMR) + docs catchup
96fe2fa Phase 4 — 4 apps migrated as Vue extensions inside Kepler
7cb16df __dirname ESM shim fix
35b0134 real handlers + settings + extension loader PoC
af24795 Phase 2 command bus full stack
8b2948f Phase 1 finishing (WS, resize, state, smoke, RAM)
0280bdb Phase 1 scaffold Electron Kepler launcher
de064e0 global swap Kepler ↔ Kosmos (836 файлов)
000034e Phase 0 + Phases 1-6 legacy Rust scaffold
```
