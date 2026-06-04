# Чек-листы по областям

Перед тем как сказать «готово» — пройди соответствующий чек-лист. По одному пункту, не пропускай.

## Я писал / правил e2e тесты (`tests/e2e/*.spec.ts`)

- [ ] `launchKepler({ slug })` — slug уникален, не пересекается с другими spec'ами.
- [ ] `KOSMOS_HEADLESS=1` не overridden в `opts.env` (стартует автоматически из helper'а).
- [ ] Backend warmup (~2.5s) перед `commands.invoke` если test чувствителен к timing.
- [ ] Locator'ы scope'ятся к специфичному CSS классу, если возможны множественные match'и (Vue transitions, panes).
- [ ] `app.close()` / `app.quit()` в `finally`.
- [ ] Если test покрывает архитектурный baseline (extension boots + commands + ARK) — это уже покрыто `extensions-contract.spec.ts` через `manifest.tests`. Не дублировать.
- [ ] Если test покрывает UI flow специфичный для extension'а — добавлен `tests` блок в manifest для contract coverage.
- [ ] Прогон в headless mode (`bun run test:e2e`) — окна не лезут на экран.

Подробнее — [Testing](/agents/testing).

## Я добавил новый extension (`extensions/<id>/`)

- [ ] `manifest.json` имеет поле `tests` (даже минимальное — `{}`).
- [ ] Если extension использует свой `object_type` — eager registration в shim на boot (не lazy перед первым upsert), иначе universal contract spec падает с FK constraint.
- [ ] Если extension'у нужен per-app UI spec — `tests/e2e/<id>.spec.ts` с helper-функцией `open<Id>(app)` для повторного использования.
- [ ] `extensions-contract.spec.ts` автоматически подхватит твой extension — прогнать `bun run test:e2e -- --grep "extension contract: <id>"`.

## Я правил ARK runtime (`crates/ark-core/rust`)

- [ ] `cargo test --manifest-path crates\ark-core\rust\Cargo.toml` — зелёный.
- [ ] `cargo build --manifest-path crates\ark-core\rust\Cargo.toml --bin ark-core-rpc` — собирается.
- [ ] `cargo build --workspace` — workspace целиком собирается (после Phase C2 общий target/).
- [ ] Если менял schema — миграция additive (`CREATE TABLE IF NOT EXISTS`), не destructive.
- [ ] Если менял sync — добавлены или обновлены тесты миграции/репликации.
- [ ] Если менял wire-протокол — остался `snake_case`.
- [ ] Self-peer filtering и routable-address filtering не ослаблены.
- [ ] `bun run --cwd packages/ark typecheck` — зелёный (если правил публичные типы).

## Я правил `@kosmos/ark` (`packages/ark`)

- [ ] `bun run --cwd packages/ark typecheck` — зелёный.
- [ ] `bun run --cwd packages/ark build` — собирается.
- [ ] `bun run --cwd packages/ark test` — зелёный (bun test).
- [ ] Если добавил новый метод — он реальный RPC к sidecar, не SDK-фильтрация.
- [ ] Self-managed и injected режимы оба работают, request id есть только в self-managed.

## Я правил Eden extension (`extensions/eden`)

- [ ] `bun run --cwd shell build:extensions` — собирается (Eden — часть extension build pipeline).
- [ ] `bun run --cwd shell typecheck` — clean.
- [ ] Все ARK операции идут через `kepler-api-shim` (`extensions/eden/src/lib/kepler-api-shim.ts`), внутри — `window.kepler.ark.request(...)`. Renderer не открывает SQLite напрямую.
- [ ] `bun run ark:guard:writes` — зелёный.
- [ ] TipTap CodeBlock + lowlight остаются для синтакс-highlight. Никаких runtime lint/format вызовов.
- [ ] Search через ARK FTS5 (`search_objects`). Heart Rust / Tantivy / ripgrep — не возвращаем.
- [ ] Lazy Editor.vue (`defineAsyncComponent`) сохранён — main bundle должен оставаться < 800KB.
- [ ] Desktop shell — через `DesktopChrome`/`DesktopContentSurface` из `@kosmos/visuals`. Никаких ручных `--titlebar-height` хаков.
- [ ] Trash UI работает поверх ARK soft-delete (`deletedAt != null`).
- [ ] Hevy / code lint-format / vault picker / Heart sidecar — **не возвращаем** в Phase 6.0.A. См. forbidden.md.

## Я правил Delphi extension (`extensions/delphi`)

- [ ] `bun run --cwd shell build:js` — собирается (extension билдится из shell сборки).
- [ ] Если правил task storage — пишет в `task_obj`, не в legacy todos.
- [ ] Не восстановлен legacy Delphi DB sidecar (DB sidecar заморожен).
- [ ] `electron-api-shim.ts` **не удалён** — он эмулирует `window.electronAPI` поверх Kepler ark bridge. Без него ломается ~30 CRUD call sites.
- [ ] Если ввёл новый `window.electronAPI.*` вызов — добавил эквивалент в `electron-api-shim.ts`.
- [ ] Tailwind plugin (`@tailwindcss/vite`) подключён в `extensions/delphi/vite.config.mjs`.
- [ ] Если правил `mobile/delphi` (Kotlin) — Android-only, не лезет в TS extension.

## Я правил Arrancador extension (`extensions/arrancador`)

- [ ] `bun run --cwd shell build:js` — собирается.
- [ ] Vue Router (memory history) routes остаются актуальными — каждый новый view зарегистрирован.
- [ ] Native scanner (`child_process` + FS-сканирование Steam/Epic/GOG) **не переписывай в renderer**.
- [ ] Game launch / catalogue / scan — TODO в extension, не возвращай stub'ы как «работающие».
- [ ] Не добавлены Tauri или React зависимости.
- [ ] ARK writes идут через `@kosmos/ark`.

## Я правил Dashboard (встроенный shell view, `shell/src/views/Dashboard*.vue` + `shell/src/dashboard/`)

- [ ] `bun run --cwd shell build:js` — собирается.
- [ ] `bun run --cwd shell typecheck` — clean.
- [ ] Renderer не открывает SQLite напрямую.
- [ ] ARK queries — только через `window.kepler.ark.request(...)`.
- [ ] Никаких writes в ARK таблицы (Dashboard — read-only inspector).
- [ ] `@kosmos/visuals` (`DesktopChrome`, `DesktopContentSurface`) через import/alias, не скопирован.
- [ ] Tray menu всё ещё содержит «Dashboard» entry.
- [ ] Закрытие dashboard окна не закрывает Kepler shell.
- [ ] Hash routing остался `#/dashboard` (без `/welcome` / `/space/<id>` — spaces убраны 2026-05-15).

## Я правил Horologion extension (`extensions/horologion`)

- [ ] `bun run --cwd shell build:js` — собирается.
- [ ] Если правил pomodoro — multi-task split в `closeArkEntry` читает АКТУАЛЬНЫЙ `pomodoroDraft`, не снапшот со старта.
- [ ] Если правил Settings — settings-окно открывается через `shell/electron/settings-window.ts` (отдельный BrowserWindow).
- [ ] Тесты — на изолированной БД (`ARK_DB_PATH=.e2e/horologion-e2e.db`).
- [ ] `time_entry_obj` и `tag_obj` — типы остаются согласованы с `crates/ark-core/rust/src/types.rs`.

## Я правил kepler-shell (`shell/`)

- [ ] `bun run --cwd shell typecheck` — clean (или `cd shell && bunx tsc --noEmit`).
- [ ] `bun run --cwd shell build:js` — clean.
- [ ] `bunx vite build --configLoader native` (внутри `shell/`) — renderer / main / preload собираются.
- [ ] Команды в `shell/electron/commands.ts` имеют корректный category (`open` / `action`); action-команды не захардкожены.
- [ ] Если правил commands — обновил `docs-site/concepts/command-bus.md`.
- [ ] Settings-окно не сломано после изменений `shell/electron/main.ts`.
- [ ] Размер окна остался fixed 720×460, без per-frame resize animation.

## Я правил extension dev mode (`shell/` + `extensions/`)

- [ ] `bun run --cwd shell dev` поднимает Akasha HMR на `:5185`; `KEPLER_DEV_EXTENSIONS=1 bun run --cwd shell dev` или `bun run --cwd shell dev:extensions` поднимают Vite dev server'ы всех Vue extension'ов на портах 5180–5185.
- [ ] Extension manifest поддерживает поле `devPort` (optional); resolver `openExtension(id, route?)` в `shell/electron/extension-host.ts` выбирает `loadURL` vs `loadFile` корректно и прокидывает `route` как hash в обоих вариантах.
- [ ] Extension source resolution probe-based: `process.env.VITE_DEV_SERVER_URL` включает TCP probe, живой `devPort` → `loadURL`, мёртвый порт → dist fallback.
- [ ] F12 toggles DevTools на любом extension window (detached, не блокирует).
- [ ] Если правил manifest format или signature `openExtension` — обновил [Extension dev mode](/concepts/extension-dev-mode) и [Extension host](/concepts/extension-host).
- [ ] Production build (без Developer Mode toggle) грузит extensions из `dist/`, не из dev server'ов.

## Я правил extension installer (`shell/scripts/install-extension.mjs`)

- [ ] `bun run --cwd shell ext:install <path|url>` отрабатывает на локальную папку и на zip-архив.
- [ ] User overrides пишутся в `%APPDATA%\Kosmos\extensions\<id>\` (не в репозиторий).
- [ ] `bun run --cwd shell ext:uninstall <id>` корректно убирает override.
- [ ] Если правил формат manifest'а — обновил [Extension installer](/concepts/extension-installer).

## Я правил command bus (`services/kepler-backend` + `@kosmos/ark`)

- [ ] Backend (`services/kepler-backend/src/command_bus.rs` + `ws_server.rs`) — `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib` зелёный.
- [ ] SDK (`packages/ark/src/ark-client.ts`) — `bun test` зелёный.
- [ ] Wire format — flat events `{event: "...", ...fields}`, согласован между backend и SDK.
- [ ] Apps register обёрнут в `try/catch` (self-managed mode без commands API — норма, не ошибка).
- [ ] Если менял публичный shape события — обновил `docs-site/concepts/command-bus.md`.

## Я правил usage-tracker module (`services/kepler-backend/src/usage_tracker/`)

- [ ] `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib` — зелёный.
- [ ] Прямые ARK writes используют `ark_core::db` хелперы.
- [ ] `lan_sync.version_vector` обновляется после прямых писей.
- [ ] Default DB path остался `%APPDATA%\Kosmos\ark.db`.
- [ ] Тесты переопределяют DB path в `.tmp` / `.e2e` / OS temp.
- [ ] Tracker-модуль стартует/останавливается из `services/kepler-backend/src/main.rs` (Phase E2). Standalone-бинарь — frozen в `legacy/usage-tracker/`.
- [ ] Tracker остаётся user-level, не Windows Service.

## Я правил focus-mode (`shell/electron/focus-*.ts` + `services/kepler-focus-*` + `services/kepler-backend/src/focus.rs`)

- [ ] `bun run --cwd shell typecheck` — clean.
- [ ] `bun run --cwd shell build:js` — clean.
- [ ] `cargo test -p kepler-focus-helper` — зелёный (`hosts.rs` unit-тесты с tempfile).
- [ ] `cargo test -p kepler-focus-svc` — зелёный (`protocol.rs` dispatch-тесты).
- [ ] `cargo build --workspace` — собирается (включая helper + svc, оба Windows-only).
- [ ] Прямые writes в hosts file идут **только** из `kepler-focus-helper` или `kepler-focus-svc` (никаких новых `fs.writeFile("C:\\Windows\\...")` в shell / backend).
- [ ] Модификации hosts остаются между маркерами `# === kepler-focus BEGIN/END ===`. Backup `hosts.kepler-backup` создаётся один раз и не перезаписывается.
- [ ] Widget operations (pause/resume/skip/stop) идут через `invokeOperation("pomodoro.<op>")`, не через локальный `setFocusState` после клика.
- [ ] Backend `focus.rs` не делает privileged operations — только хранит state.
- [ ] Если правил `setupFocusWidgetBackendSync` / `teardownFocusWidgetBackendSync` — wiring в `main.ts` зовёт teardown перед resubscribe при backend respawn.
- [ ] Если правил pipe protocol (`kepler-focus-svc/src/protocol.rs`) — request/response shape остаётся backward compatible (shell может говорить со старой версией service'а и наоборот).
- [ ] Если менял auto-install flow — `autoInstallAttemptedThisSession` + `setFocusServiceAutoInstallDeclined` гварды не ослаблены (один UAC промпт максимум).
- [ ] Если добавил новую `focus.*` ARK операцию — диспатч в `ws_server.rs` + middleware в `extension-host.ts` (если требует apply на hosts).
- [ ] `requireAdministrator` manifest у `kepler-focus-helper.exe` на месте (`build.rs` embed-manifest).
- [ ] Headless e2e не показывает widget (`process.env.KOSMOS_HEADLESS === "1"` гвард в `showWidget`).

См. [Focus mode](/concepts/focus-mode).

## Я правил `@kosmos/visuals` (`packages/visuals`)

- [ ] Не сломан public API (`index.ts` экспортирует те же имена).
- [ ] Если меняешь CSS-переменные в `theme/css-variables.css` — сразу отрази в `docs-site/.vitepress/theme/custom.css`.
- [ ] `theme/css-variables.css` остаётся source of truth для theme tokens; TypeScript exports не должны расходиться с CSS variables.

## Я правил мобильный код (`mobile/`)

- [ ] `mobile/delphi/` — Kotlin Room. Изменения в схеме согласованы с `mobile/ark-service/` ContentProvider.
- [ ] `mobile/ark-service/` — ContentProvider публикует только то, что приложение само пишет. Чужие writes не разрешены.

## Я правил документацию (`docs-site/`)

- [ ] `bun run docs:build` — собирается.
- [ ] `bun run docs:check` — зелёный (нет stale references).
- [ ] Все внутренние ссылки рабочие.
- [ ] Русский язык, без английских заглушек.
- [ ] Не дублирую правила, лучше ссылка на канон-страницу.
- [ ] Запустил `bun run docs:sync` для регенерации AGENTS.md / CLAUDE.md / llms.txt.

## Я делал `LIGHT_LOOP`

- [ ] Классификация явно названа как `LIGHT_LOOP`, и задача не содержит триггеров `FULL_LOOP`.
- [ ] Если появились ARK/data/sync/schema/write-boundary/focus/command bus/security/architecture признаки — задача эскалирована в `FULL_LOOP`.
- [ ] Прогнаны минимальные релевантные проверки для затронутой области.
- [ ] Для UI/visual правки сделан visual verify, screenshot сохранён под `.tmp/`.
- [ ] В финальном отчёте указано, что проверено и что не проверено.

## Я делал `NO_LOOP`

- [ ] Правка действительно trivial/edit-level и не меняет правила, архитектуру, data path или user workflow.
- [ ] Прогнана релевантная быстрая проверка или честно указано, почему она не нужна.

## Я делал `FULL_LOOP` / substantial задачу через proof loop

- [ ] Создан `.agent/tasks/<DATE>-<slug>/spec.md` **до** реализации.
- [ ] `spec.md` содержит явные AC1..ACn, проверяемые утверждения.
- [ ] `evidence.md` со ссылками на `raw/<cmd>.md` логи.
- [ ] `evidence.json` с PASS/FAIL по каждому AC.
- [ ] Свежая верификация после `evidence.md` — против текущего кода.
- [ ] Если был FAIL — `problems.md` + минимальный fix + reverify.
- [ ] **Каждый** AC = PASS, иначе не клейми «готово».

## Общий финальный чек-лист

- [ ] `bun run ark:guard:writes` — зелёный (если трогал data-слой).
- [ ] `bun run ark:smoke` — зелёный (для substantial задач).
- [ ] Коммит-сообщение осмысленное (про **почему**, не «add big»).
- [ ] Не амендил опубликованные коммиты.
- [ ] Не использовал `--no-verify` для коммита.
- [ ] Все новые `.tmp` / `.e2e` / `dist/` пути в `.gitignore`.
