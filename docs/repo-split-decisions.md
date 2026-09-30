# Разделение репозиториев: решения

Рабочий журнал решений по выносу лишнего из cortex. Применяется в ветке
`kos-137` (база `942acb9a`) в конце обсуждения.

**Статус (KOS-236):** Electron-оболочка и Electron Package Host удалены из
репозитория. В продакшене для Windows остаются Engine (`mundus-engine` с
in-process ARK, трей + апдейтер + автостарт) и GPUI-компоненты под
`resources/components/<name>/`. Упомянутые в этом журнале Electron-файлы
(`desktop/electron/`, `host/electron/`, `desktop/src/`, Vue-UI и e2e) являются
историческими записями и в дереве больше не существуют.

## Принципы

- Один репо = одна граница релиза и одна граница контекста агента.
- Cortex = одно приложение: **UI + core**. `core/` вмёржен в cortex намеренно
  (subtree, KOS-129) — core и дашборд сливаются в одно приложение.
- Manager — часть этого приложения, не отдельный продукт. Экран Data
  (браузер объектов) уже реализован в `manager-gpui` (`src/views/data.rs`).
- Инкубатор (`makekosmos/incubator`) — только начатое и временно замороженное.
  Мёртвое удаляется (история — в git-тегах), чужое уезжает к владельцу.
- `docs/gpui-host-decision.md` не является источником решений.
- **Приложения взаимодействуют с ОС только через Engine** и права манифеста.
  Привилегированные возможности (блокировка фокуса, микрофон, вставка текста,
  хоткеи) принадлежат cortex и выдаются пакетам через права.

## Решения

| # | Элемент | Решение | Статус |
|---|---|---|---|
| 1 | `manager/` (Vue Manager) | **Удалить** вместе со всеми ссылками (typecheck, check-plan, source-size, layout, release-preflight, linux-smoke, e2e, `MUNDUS_MANAGER_MAIN`). Для перехода на GPUI не нужен | Готово в worktree `C:\mk\cortex-rm-vue-manager` (ветка `chore/remove-vue-manager`), не закоммичено; перенести в `kos-137` |
| 2 | `manager-gpui/` | **Остаётся в cortex** как основа единого приложения | Решено |
| 3 | `packages/*` (bigfrontend, codewars, greatfrontend, hevy, huawei-health, leetcode, toggl, raycast-api, ark-markdown-bridge) | Сторонние пакеты, все **живые**. Вынести из cortex; обновляться независимо от cortex | Решено, репо — см. 3b |
| 3b | Куда выносить пакеты | Один репо **`integrations`** (новый, **open source**): каждый пакет со своим `manifest.json` и своей версией | Решено |
| 4 | Vue-дашборд в `desktop/` (`views/Dashboard*`, `dashboard/`, `body/`, `coder/`, `integrations/`, `my-cosmos/`) | Data — уже в `manager-gpui`. **Usage → перенести в `manager-gpui`**. **my-cosmos → инкубатор**. body/coder/integrations уходят вместе с пакетами (п.3). После этого Vue-дашборд удалить целиком | Решено |
| 5 | Launcher (`desktop/src/views/LauncherView.*`, `desktop/electron/main-launcher.ts`, Alt+Space) | Отдельное приложение → **инкубатор** (Vue-порт уже в `incubator/launcher`). Из cortex удалить | Решено |
| 5a | Точки входа в Agenda | Agenda — полноценное отдельное приложение: открывается через меню Пуск, ярлык и т.п., а также из дашборда (`manager-gpui`). Через лаунчер — нет | Решено |
| 6 | Focus | **UI фокуса в cortex не нужен** — фокусом (включая pomodoro) будет **ordo**. Блокировка и Engine-операции `focus.*`/`pomodoro.*` **остаются в cortex** — это привилегированная возможность хоста, приложения взаимодействуют с ОС только через Engine и права манифеста | Решено |
| 8 | Package Host и легаси extension-host | Легаси `.kext`-рантайм в `desktop/` (**~45 файлов `extension-*`**) — **удалить целиком**. Dictation уходит в GPUI-модуль, launcher заморожен в инкубаторе. Рантайм пакетов один — `host/` (.kspkg). Проверить, нужен ли одноразовый мигратор данных для уже установленных `.kext` | Решено |
| 9 | Канал для Agenda-GPUI | Остаётся **компонентом установщика** (версия с Mundus). Roadmap: kind «нативное приложение» в `.kspkg`/Package Index для независимых обновлений — **KOS-138** (Backlog) | Решено |
| 10 | `kosmos-gpui-kit` | **Отдельный репо** по образцу imago («imago для GPUI»): тема, виджеты, Engine-клиент из `manager-gpui`. NB: в imago Rust-крейта нет, он TS-only — kit будет новым репо с Rust-crate | Решено |
| 11 | Мусор в `core/` subtree | **Чистим в cortex сейчас** + отдельно предлагаем ту же чистку в upstream `makekosmos/core`, чтобы не вернулась при подтягивании | Решено |
| 7 | Dictation | GPUI, **модуль единого приложения cortex** (не отдельный бинарь): пилюля — окно того же GPUI-процесса, настройки — его экран. Весь Vue-UI диктации (`desktop/electron/dictation-pill.ts`, `desktop/src/views/DictationPillView.vue`, `dictation-pill-waveform.ts`, `dictation-model-selection.ts`, `DictationTab.vue`, `useDictationConfig*`, `useDictationPending.ts`) уходит из `desktop/`. Функциональные возможности (`runtime/src/dictation/`: микрофон, модели, транскрибация, хоткей-хук, вставка текста) остаются в cortex и выдаются через права | Решено (модуль cortex-GPUI; старый пакет `makekosmos/dictation` выводится) |

## Заметки к п.3b (open source)

- Перед публикацией: проверить историю на секреты, выбрать лицензию.
- Исследовательские материалы (reverse-engineering и т.п.) — в специальную
  папку, закрытую `.gitignore` в репо `integrations`: в git только
  функциональный код, нужный для работы пакета. Первый кандидат —
  `packages/huawei-health/REVERSE_ENGINEERING.md`.
- `REVERSE_ENGINEERING.md` уже есть в истории cortex, поэтому переносить
  `huawei-health` в открытый репо **без истории** (или с отфильтрованной историей).

## Заметки к п.5 (Launcher)

- Статические команды шелла из `desktop/electron/commands.ts` (`settings:open`,
  `mundus:agenda-gpui`, focus, dictation) вызываются через лаунчер; после его
  удаления у них остаются только другие точки вызова.
- `commands.ts` используется не только лаунчером: `main-runtime-integrations.ts`
  берёт оттуда `setDictationShortcutResolver`, `main-commands.ts` — `COMMANDS`.
  Удалять только лаунчер-специфичное.

## Заметки к п.6 (Focus) — что сейчас лежит в cortex

- UI и логика сессий в шелле (уходит, дублирует ordo):
  `desktop/electron/focus-*` + `pomodoro-notifier.ts` (22 файла);
  `desktop/src/views/FocusWidgetView.vue`, `FocusBlockOverlay.vue`,
  `components/FocusCommandPanel.vue`, `focusCommandPayload.ts`,
  `lib/focusAppBlocking.ts`, `lib/focusLauncherCommands.ts`,
  вкладка настроек `views/settings/tabs/Focus*` + `composables/useFocusTab*`.
- Engine: `runtime/src/focus.rs`, `runtime/src/pomodoro/**`, `runtime/src/pomodoro_host.rs`
  — операции, которые ordo уже вызывает (`focus.*`, `pomodoro.*`).
- Блокировка: `native-services/focus-svc`, `focus-helper`,
  `watcher` (18 файлов), клиент `shared/focus-service-*`.

## Заметки к п.7 (Dictation)

- Контракт в `dictation/AGENTS.md` сейчас говорит обратное: «overlay lifecycle
  ... remain in Cortex». Его нужно обновить вместе с переносом.
- Пилюля — always-on-top оверлей, который сейчас создаёт Electron main
  (`dictation-pill.ts`). Чтобы пакет владел её UI, хосту нужна возможность
  «оверлей-окно пакета» через права — проверить, есть ли она, иначе добавить.
- Общий принцип (из этого решения): **UI — в приложении, функциональные
  возможности — в cortex, переиспользуемые через права манифеста.**

## Что нужно для независимого обновления пакетов (п.3)

Дистрибуция уже независима: у каждого пакета свой `.kspkg`, своя запись в BOM
`package-index`, `kind: integration` в `store/catalog.json`, контракт
`engine_api` в манифесте. Привязка к cortex осталась в:

1. Исходниках: `cortex/packages/`, BOM ссылается на `repository: makekosmos/cortex`.
2. Сборщике `package-index/scripts/build-source-packages.mjs`: требует `--cortex`,
   читает `packages/<provider>`, импортирует `desktop/scripts/zip-utils.mjs` из cortex.
3. `packages/ark-markdown-bridge/Cargo.toml`: `mundus-engine = { path = "../../runtime" }`
   — заменить на тонкий протокольный crate. Остальные 7 пакетов — самостоятельные
   cargo-проекты без path-зависимостей.
4. Пакетно-специфичном коде хоста: `runtime/src/package_service/integrations/huawei_login.rs`,
   `integrations.rs`, `desktop/electron/leetcode-auth-flow.ts`, `leetcode-integration.ts`.
   Логин должен работать только по контракту манифеста (`integration.login`,
   `settings[].injection`) через общий login broker хоста.

## Заметки к п.4 (Usage)

- Vue-источник: `desktop/src/dashboard/store.ts` — операции `get_usage_analytics`
  и `app_index.list_all`, UI — `desktop/src/dashboard/UsageTable.vue`.
- **Проверено (разведка E3):** обе операции доступны клиенту `manager-gpui` через
  `POST /v1/rpc` без дополнительных грантов — гейтинг по ним живёт только в
  Electron-слое (`extension-permissions.ts`), Engine пропускает. `app_index.*`
  обрабатывается напрямую, `get_usage_analytics` уходит через ark_host во
  встроенный ARK (раньше — в sidecar `ark-core-rpc`, больше не поставляется).
  Параметры: `{range_days, top_apps_limit, recent_sessions_limit}`
  и `{limit}`.
- ⚠️ Иконки: `app_index.list_all` возвращает `icon_ref: "mundus-icon://app/<id>"`
  — это Electron-протокол, GPUI его не загрузит. Для иконок в Usage-таблице
  использовать `app_index.icon_path` (реальный путь на диске).

## Открытые пункты

- Проверка перед удалением легаси extension-host (п.8): есть ли у пользователей
  установленные `.kext`, которым нужен мигратор данных.

## План исполнения

Ограничения:
- `kos-137` — ветка интеграции; идёт смок кандидата 0.9.38, работа ведётся в
  отдельных worktree, слияние в `kos-137` в конце.
- Один логический change = один коммит/ветка. Новые репо сначала регистрируются
  в `docs/governance/repository-lifecycle.md` + `repository-bom.json` (правило 4).
- Каждый сабагент — отдельный worktree своего репо, непересекающиеся файлы,
  самодостаточный AGENTS.md; возвращает SHA/дифф + PASS/FAIL/NOT_RUN.
  Лид интегрирует последовательно и проверяет финальный дифф.
- Профили: `subagent_explore` — только разведка (read-only);
  `subagent_general` — правки и проверки в собственном worktree.
- Создание репо в организации и публикация — действия пользователя/лида,
  не сабагентов.

### Волна 0 — сейчас, параллельно (разведка + governance)

| Агент | Задача | Тип |
|---|---|---|
| E1 | Классификация `desktop/electron/extension-*` + миграция `.kext` — **DONE**, выводы ниже | done |
| E2 | Карта удалений фазы 2: все ссылки на Launcher, Vue-дашборд, focus-UI, dictation-UI за пределами их каталогов (commands.ts, main.ts, settings, e2e, check-* скрипты) | explore |
| E3 | Проверка `/v1/rpc`: доступны ли `get_usage_analytics`, `app_index.list_all` клиенту manager-gpui (гранты/неймспейсы) | explore |
| G1 | docs repo: строки lifecycle + BOM для `integrations` и `kosmos-gpui-kit` — **DONE**, `C:\mk\docs-repo-register`, ветка `chore/register-new-repos`, не закоммичено, `bun run check` PASS. Оба репо — `reserved`, null-OID как placeholder ревизии; правки также в `repository-quality-adoption.md` и `validate-governance.mjs` (35→37) | done |

Фаза 0 (manager/) уже готова: `C:\mk\cortex-rm-vue-manager`, `chore/remove-vue-manager` — закоммитить при старте работ.

### Волна 1 — границы репозиториев (после G1, параллельно по репо)

| Агент | Задача | Тип |
|---|---|---|
| W1 | package-index: `build-source-packages.mjs` без `--cortex` — читает `repository`/`ref` из BOM, `zip-utils` переезжает в репо | general, worktree package-index |
| W2 | cortex: убрать `path = "../../runtime"` у ark-markdown-bridge → протокольный crate по пину | general, worktree cortex |
| W3 | incubator: перенос `my-cosmos` (desktop/src/my-cosmos + electron/my-cosmos-window.ts) как замороженный снапшот | general, worktree incubator |
| W4 | cortex: чистка `core/` subtree (incubator, fatsecret, plan-md, png, bun.lock, playwright.config) | general, worktree cortex |

Создание репо: `makekosmos/integrations` **создан** — private (public после
ревью истории/секретов и выноса huawei research), `main` @ `e0168cf`, локально
`C:\Users\kirill\Coding\makekosmos\integrations`; `.gitignore` уже содержит
`research/` для RE-материалов. `makekosmos/kosmos-gpui-kit` **создан** —
private, `main` @ `80ced30`, локально `C:\Users\kirill\Coding\makekosmos\kosmos-gpui-kit`.
Решено подтверждено: imago не подходит (TS/Vue-only, zero `.rs`), kit —
отдельный Rust-репо. В `docs` (ветка `chore/register-new-repos`, коммит
`927eb1e`) оба репо записаны с наблюдаемыми ревизиями вместо null-OID.
Затем:
- перенос `packages/*` в integrations (huawei-health без истории), перепин BOM;
- удаление `packages/` из cortex + правки check-plan/source-size;
- вынос theme/widgets/engine/fields из manager-gpui в kosmos-gpui-kit, перепин manager-gpui и agenda-gpui.

### Волна 2 — удаления в cortex (после смока; ветки мёржатся последовательно)

| Агент | Задача | Зависит от |
|---|---|---|
| D1 | Удалить лаунчер (LauncherView, main-launcher, лаунчер-часть commands.ts); сохранить COMMANDS/setDictationShortcutResolver | E2 |
| D2 | Удалить focus-UI (FocusWidgetView, FocusCommandPanel, lib/focus*, вкладка Focus, electron/focus-session-*, pomodoro-notifier). Остаются: native-services, shared/focus-service, runtime focus.rs/pomodoro, FocusBlockOverlay | E2 |
| D3 | Порт Usage → manager-gpui (`get_usage_analytics` + `app_index.list_all`, UI по мотивам UsageTable.vue) | E3 |
| D4 | Удалить Vue-дашборд целиком + body/coder/integrations + dashboard-window + leetcode-* | D3 |

### Волна 3 — GPUI-приложение (архитектура у лида, порты агентам)

- S1: Windows bring-up GPUI (multi-window, transparent always-on-top overlay, IME, HiDPI) — спайк, решает пилюлю и focus-оверлей.
- S2: модуль диктации (пилюля-окно + экран настроек) — после S1; затем удаление Vue-UI диктации из desktop/.
- S3: порт настроек шелла (SettingsView → GPUI) — последний крупный Vue-остаток.
- S4: focus-оверлей в GPUI — после S1.

### Волна 4 — удаление легаси extension-host

Классификация выполнена (E1). Ключевой факт: `.kspkg`-путь не проходит через
`desktop/electron/extension-*` — установка/хостинг лежат в `runtime/src/package_*`
(операции `packages.*`) и `host/electron` (Package Host). Значит:

- **Удалять** (~25 файлов): весь Phase-4 рантайм — `extension-host.ts`,
  `extension-preload.ts`, `extension-browser-window.ts`, `extension-window-*`,
  `extension-ark-ipc` (сначала убрать 3 вызова из `main-ark-client-controller.ts`),
  `extension-native-runner`, `extension-declared-commands`,
  `extension-user-data-ipc` (НЕ путать с `host/electron/extension-user-data-ipc.ts` —
  это другой файл для .kspkg), `extension-markdown-*`, `extension-book-metadata-ipc`,
  `extension-image-color-ipc`, `extension-installer*`, `extension-marketplace`,
  `extension-update-plan`, `extension-zip`, `extension-manifest*`,
  `extension-package-registry`, `extension-display-name`, `install-extension-window.ts`
  + их тесты. NB: `extension-host.ts` уже подгружается только лениво из
  `legacy-migration-runtime.ts` — убрать тот импорт вместе.
- **Оставить**: `extension-data-migration.*` (merge-примитив живого мигратора)
  и `extension-permissions.*` (миска: общие `JsonValue/isRecord/isString` +
  `ExtensionSource` импортируют ~20 живых файлов — вынести типы в отдельный
  модуль, удалить только legacy-specific `assertExtension*Permission`).
- **Соседний легаси вне glob** (тоже в удаление): `command-host/*`,
  `main-command-host.ts`, `main-benchmark-open-all.ts`, `main-initial-kext.ts`,
  `mundus-api.ts`, `local-image-protocol.ts` (electron-копия),
  `book-metadata-{browser,fetch,open-library,connect-proxy}.ts`,
  `image-{dominant-color,dimensions}.ts`, `public-network-address.ts`,
  `InstallExtensionView.vue` + роут `#install-extension`, `mundus.extension.*`
  в `preload-bridge.ts`/`shared/ipc-api-types.ts`, `.kext`-скрипты
  (`install-extension.mjs`, `uninstall-extension.mjs`, `publish-extension.mjs`,
  `generate-catalog.mjs`, `extension-package-utils.mjs`, `vite.extensions.config.mjs`),
  e2e `kext-{install,revert,argv}.spec.ts`, `extension-api-compat.spec.ts`.
- **Тулинг**: root `package.json` `test:package-contract` указывает на
  `extension-manifest-validation.test.ts`; `check-source-size.mjs` whitelist —
  обновить.
- **Миграция данных**: одноразовый мигратор уже есть (`legacy-migration-*` +
  `extension-data-migration`), но его allowlist — только arcadia/eden/delphi →
  .kspkg. Данные прочих `.kext` в `<dataDir>/extensions-data/<id>` просто
  осиротеют (не удаляются) — это приемлемо, зафиксировано.
  ⚠️ Сам каталог `extensions-data/` удалять нельзя: туда пишет AgentsService
  (`extensions-data/daedalus/daedalus.db`).

### Волна 5 — закрытие

- Обновить lifecycle/BOM, AGENTS.md (cortex/ordo/dictation), store refs, README.
- Интеграция в `kos-137`, `pnpm run check`, финальный смок.

## Ход исполнения (ledger)

| Дата | Шаг | Результат |
|---|---|---|
| 2026-09-24 | Фаза 0: удаление `manager/` | `chore/remove-vue-manager` @ `a14eb104`, полный гейт PASS (host e2e 6/6 после провижининга sibling-фикстур в `C:\mk`) |
| 2026-09-24 | G1: docs governance | `chore/register-new-repos` @ `927eb1e` — integrations + kosmos-gpui-kit записаны с реальными ревизиями, `bun run check` PASS |
| 2026-09-24 | Репо созданы | `makekosmos/integrations` private @ `e0168cf`; `makekosmos/kosmos-gpui-kit` private @ `80ced30` |
| 2026-09-24 | W5: пакеты → integrations | `66f9040` pushed; huawei RE-материалы → gitignored `research/`; секретов не найдено; raycast-api — helper без manifest (не .kspkg) |
| 2026-09-24 | W3: my-cosmos → incubator | `chore/my-cosmos-snapshot` @ `c322cd7`; в cortex my-cosmos уже мёртв (нет роута/импортеров) |
| 2026-09-24 | W1: package-index decouple | `chore/bom-source-build` @ `c1f45a3` (содержит `97200b5` + repin BOM → integrations). `bun run check` PASS 29/29 |

| 2026-09-24 | W2: ark-markdown-bridge decouple | cortex `chore/worker-protocol-crate` @ `7d99757` (crate `package-protocol`); integrations @ `1eefaa3` pushed (git+rev pin ждёт мержа cortex-ветки) |
| 2026-09-24 | W6: kosmos-gpui-kit extraction | kit @ `268ffed` pushed (theme/engine/fields/widgets); cortex `chore/gpui-kit-consume` @ `81afa0fa` — manager-gpui потребляет kit по пину |
| 2026-09-24 | W4: core/ subtree cleanup | `chore/core-subtree-clean` — `73b6db55` + `68691471`, full gate PASS |
| 2026-09-24 | packages/ + command-host + my-cosmos удалены | `chore/remove-packages` — `2de0411e` + `d4451ab4`; command-host пришлось удалить: зависел от `packages/raycast-api` (`@raycast/api`); манифесты пакетов → `runtime/tests/fixtures/` |
| 2026-09-24 | Интеграция волны 1 | все 4 cortex-ветки смёржены в `chore/remove-vue-manager` → `c2beed29`, конфликтов нет; комбинированный `pnpm run check` запущен |

| 2026-09-24 | Комбинированный check на `c2beed29` | `pnpm run check` PASS (полный гейт на объединённом дереве волны 1) |
| 2026-09-24 | D3: Usage порт в manager-gpui | `31d8f715` смёржен → `b02d8aef`; `views/usage.rs` + `get_usage_analytics`/`app_index.*` через kit engine client; гейт PASS |

| 2026-09-24 | D3.5: Dictation GPUI-модуль | `8dae4127` смёржен → `00ef9ac1`; `views/dictation{,_cards}.rs` + `pill.rs` (PopUp overlay, реальный always-on-top); весь `dictation.*` ops через `/v1/rpc` |
| 2026-09-24 | dictation AGENTS.md | `chore/agents-gpui-move` @ `5a7df7c` в репо dictation — UI переезжает в cortex, репо заморожен |

- Dictation GPUI: глобальный хоткей не работает — `hotkey_hook` шлёт события по Engine WS, а `kosmos_gpui_kit::engine` HTTP-only. **Блокер для удаления Vue-пилюли**: нужен WS-subscriber в kit. Также нет waveform-уровней (WASAPI на engine) и прогресса скачивания моделей (WS-события).
- ureq timeout 15s в kit engine client — длинные транскрипции могут падать client-side (pending-item сохраняется, retry работает).

| 2026-09-25 | D1: Launcher удалён | `abeae56e` → merge `b47bb1c9`; launcher surface (LauncherView, main-launcher, post-update, App.vue, assets, launcher commands) убран; COMMANDS/openHostedApp сохранены |
| 2026-09-25 | D2: Focus UI удалён | `5d18f38a` → merge `a1191eb1`; весь `desktop/electron/focus-*` UI + FocusWidget/BlockOverlay/FocusTab убраны; `focus-block.ts`/`focus-enforcement.ts` (OS-блокировка через Helper) и Engine ops `focus.*`/`pomodoro.*` сохранены |
| 2026-09-25 | D4: Vue Dashboard удалён | `17f6d975` → merge `4fb17d4f`; dashboard/, body/, coder/, integrations/, leetcode host-код, huawei_login/cleanup в runtime; generic credential machinery сохранён |
| 2026-09-25 | WS-подписка в kit | kit @ `b5f9b93` (engine_ws); cortex `ab0090d6` → merge `302892f5`; хоткей → pill работает через Engine WS |

| 2026-09-25 | W4x: `.kext`-рантайм удалён | `4c6265d` merge — Phase-4 extension runtime убран (~67 файлов), мигратор и `extensions-data/` сохранены, shared-утилиты → `json-types.ts` |
| 2026-09-25 | Волна 2+4 завершена | Все removal-ветки смёржены в `chore/remove-vue-manager`; desktop/ сокращён до host-инфра + SettingsView + DictationPillView |

- Dictation Vue pill (`desktop/src/views/DictationPillView.vue` + `dictation-pill.ts`) — GPUI-модуль с хоткеем работает, но waveform и прогресс скачивания моделей — только в Electron. Пилюлю не удалять до parity (нужен level-feed с engine или отдельное решение).
- Shell settings (`SettingsView.vue`) — последняя Vue-поверхность; уходит когда GPUI настройки будут портированы (волна 3, след. задача).

| 2026-09-25 | Финальный check | `pnpm run check` на объединённом дереве — PASS; каждый merge-коммит проходил полный check-plan с host e2e |
| 2026-09-25 | Интеграция в kos-137 | `chore/remove-vue-manager` смёржен в локальный `kos-137`; дерево байт-идентично проверенному (`ffc6a13`). Не запушено — требуется твоё решение |

### Известные follow-up долги

- `publish-package-v1.yml` workflow: для fetch из private integrations нужен `gh auth setup-git`/токен (workflows не трогаем — отдельная задача).
- ~~`publish-catalog.mjs` пока использует cortex-скрипты `package-sign`/`package-envelope`/`package-catalog`~~ — закрыто: package-index подписывает каталог своими скриптами, мёртвые cortex `package-*` удалены в KOS-274.
- ark-markdown-bridge: pin `git+rev` на cortex активируется после мержа `chore/worker-protocol-crate` в kos-137 и пуша.
- Junction `C:\mk\cortex` → `cortex-rm-vue-manager` — нужен host e2e; снять после интеграции (может перенаправить чужие тесты).
- E2E-фикстуры `C:\mk\{agenda,arcadia,dictation,memoria,ordo}` — detached worktrees для тестов; держать пока идёт ветка.
