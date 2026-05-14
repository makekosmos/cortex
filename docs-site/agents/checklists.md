# Чек-листы по областям

Перед тем как сказать «готово» — пройди соответствующий чек-лист. По одному пункту, не пропускай.

## Я правил ARK runtime (`packages/ark-core/rust`)

- [ ] `cargo test --manifest-path packages\ark-core\rust\Cargo.toml` — зелёный.
- [ ] `cargo build --manifest-path packages\ark-core\rust\Cargo.toml --bin ark-core-rpc` — собирается.
- [ ] Если менял schema — миграция additive (`CREATE TABLE IF NOT EXISTS`), не destructive.
- [ ] Если менял sync — добавлены или обновлены тесты миграции/репликации.
- [ ] Если менял wire-протокол — остался `snake_case`.
- [ ] Self-peer filtering и routable-address filtering не ослаблены.
- [ ] `bun run --cwd packages/kosmos-ark typecheck` — зелёный (если правил публичные типы).

## Я правил `@kosmos/ark` (`packages/kosmos-ark`)

- [ ] `bun run --cwd packages/kosmos-ark typecheck` — зелёный.
- [ ] `bun run --cwd packages/kosmos-ark build` — собирается.
- [ ] Если добавил новый метод — он реальный RPC к sidecar, не SDK-фильтрация.
- [ ] Self-managed и injected режимы оба работают, request id есть только в self-managed.

## Я правил Eden (`apps/eden/ts`)

- [ ] `bun run --cwd apps/eden/ts build` — собирается без ошибок.
- [ ] `bun run --cwd apps/eden/ts test:e2e` — зелёный.
- [ ] `bun run --cwd apps/eden/ts lint` — без warnings.
- [ ] `bun x tsc --noEmit` (в `apps/eden/ts`) — зелёный.
- [ ] Не возвращён ripgrep, поиск через Heart/Tantivy / ARK FTS.
- [ ] Если трогал `main/store.ts` — hardening для `save/move/delete` не сломан.
- [ ] Desktop shell — через `DesktopChrome`/`DesktopContentSurface` из `@kosmos/visuals`. Никаких ручных `--titlebar-height` хаков.
- [ ] Если трогал тесты — изолированная БД, не user vault.

## Я правил Delphi (`apps/delphi/ts`)

- [ ] `bun run --cwd apps/delphi/ts build` — собирается.
- [ ] `bun run --cwd apps/delphi/ts test` — зелёный.
- [ ] `bun run --cwd apps/delphi/ts e2e` — зелёный.
- [ ] Если правил task storage — пишет в `task_obj`, не в legacy todos.
- [ ] Не восстановлен legacy Delphi DB sidecar.
- [ ] Тесты — на изолированной БД.

## Я правил Arrancador (`apps/arrancador`)

- [ ] `bun run --cwd apps/arrancador typecheck` — зелёный.
- [ ] `bun run --cwd apps/arrancador build:renderer` / `build:main` / `build:preload` — собираются.
- [ ] `bun run --cwd apps/arrancador test` — зелёный.
- [ ] `bun run --cwd apps/arrancador smoke:packaged` — зелёный (если правил packaging / Electron main).
- [ ] Не добавлен in-process tracker / window polling / app-owned usage SQLite.
- [ ] Не добавлены Tauri или React зависимости.
- [ ] ARK writes идут через `@kosmos/ark`.
- [ ] Read-only SQLite — только fallback, отделён от write paths.

## Я правил Dashboard (`apps/dashboard`)

- [ ] `bun run --cwd apps/dashboard build` — собирается.
- [ ] `bun run --cwd apps/dashboard test:e2e` — зелёный.
- [ ] `bun run --cwd apps/dashboard smoke:seed` + `smoke:analytics` — зелёные.
- [ ] Renderer не открывает SQLite напрямую.
- [ ] ARK queries только в `electron/services/analytics.ts`.
- [ ] Никаких writes в ARK таблицы.
- [ ] `@kosmos/visuals` через import/alias, не скопирован.

## Я правил kepler-shell (`apps/kepler-shell`)

- [ ] `bun run --cwd apps/kepler-shell typecheck` — clean.
- [ ] `bun run --cwd apps/kepler-shell build:js` — clean.
- [ ] `bun x vite build --configLoader native` — все 3 environments (renderer / main / preload) собираются.
- [ ] Команды в `electron/commands.ts` имеют корректный category (`open` / `action`); action-команды не захардкожены, приходят dynamic от приложений.
- [ ] Если правил commands — обновил `docs-site/concepts/command-bus.md`.
- [ ] Settings-окно не сломано после изменений `main.ts`.
- [ ] Extension PoC всё ещё открывается (`dashboard:extension:demo` команда работает).
- [ ] Размер окна остался fixed 720×460, без per-frame resize animation.

## Я правил extension dev mode (`apps/kepler-shell` + extensions)

- [ ] `KEPLER_DEV=1` + `bun run --cwd apps/kepler-shell dev:extensions` поднимают Vite dev server на каждом из портов 5180–5183.
- [ ] Extension manifest поддерживает поле `devPort` (optional); resolver `openExtension(id)` в `electron/extension-host.ts` выбирает `loadURL` vs `loadFile` корректно.
- [ ] F12 toggles DevTools на любом extension window (detached, не блокирует).
- [ ] Settings → Developer Mode toggle persist в `%APPDATA%\Kosmos\kepler-shell-settings.json`.
- [ ] Если правил manifest format — обновил [Extension dev mode](/concepts/extension-dev-mode) и [Extension host](/concepts/extension-host).
- [ ] Production build (без `KEPLER_DEV`) грузит extensions из `dist/`, не из dev server'ов.

## Я правил Delphi extension (`apps/kepler-shell/extensions/delphi`)

- [ ] `electron-api-shim.ts` **не удалён** — он эмулирует `window.electronAPI` и нужен для compat с legacy Delphi-кодом. Без него ломается CRUD во всём приложении (требует переписывания каждого call site).
- [ ] Tailwind plugin (`@tailwindcss/vite`) подключён в `vite.config.mjs` extension'а. Без него страницы Delphi теряют классы.
- [ ] Если ввёл новый `window.electronAPI.*` вызов в Vue-коде — добавил эквивалент в `electron-api-shim.ts` (через kepler ark bridge).
- [ ] Миграция UI на plain CSS / kosmos-visuals токены — **Phase 9, отдельная задача**. Не делай попутно с другими правками.
- [ ] `bun run --cwd apps/kepler-shell build:js` — собирается без ошибок.
- [ ] При запуске Kepler shell extension открывается, без crash'а на missing window.electronAPI.

## Я правил Arrancador extension (`apps/kepler-shell/extensions/arrancador`)

- [ ] Vue Router (memory history) routes остаются актуальными — каждый новый view зарегистрирован в роутере.
- [ ] Native scanner (`child_process` + FS-сканирование Steam/Epic/GOG) **не переписывай в renderer** — он живёт в legacy standalone Arrancador main process (Phase 5+ план — миграция в kepler-backend Rust либо в kepler-shell sidecar, см. [Decisions](/reference/decisions#2026-05-14-arrancador-native-scanner-остался-в-legacy)).
- [ ] Game launch / catalogue / scan — **TODO в extension**, не возвращай случайно stub'ы как «работающие» (только UI subset мигрирован: LayoutPage + GameCard).
- [ ] `bun run --cwd apps/kepler-shell build:js` — собирается.
- [ ] `electron-api-shim.ts` или эквивалент (если используется) — не сломан после правок.

## Я правил command bus (`services/kepler-backend` + `@kosmos/ark`)

- [ ] Backend (`services/kepler-backend/src/command_bus.rs` + `ws_server.rs`) — `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib` зелёный.
- [ ] SDK (`packages/kosmos-ark/src/ark-client.ts`) — `bun test` зелёный.
- [ ] Wire format — flat events `{event: "...", ...fields}`, согласован между backend и SDK.
- [ ] Apps register обёрнут в `try/catch` (self-managed mode без commands API — норма, не ошибка).
- [ ] Если менял публичный shape события — обновил `docs-site/concepts/command-bus.md`.

## Я правил Horologion (`apps/horologion`)

- [ ] `bun run --cwd apps/horologion typecheck` — зелёный.
- [ ] `bun run --cwd apps/horologion build:js` — собирается (sidecar + tsc + vite).
- [ ] `bun run --cwd apps/horologion test:e2e` — зелёный (selectors из `HomeView`/`StopwatchView`/`PomodoroView` актуальны).
- [ ] Если правил pomodoro — multi-task split в `closeArkEntry` читает АКТУАЛЬНЫЙ `pomodoroDraft`, не снапшот со старта.
- [ ] Если правил Settings — IPC `horologion:settings:open` в `main.ts` + preload + `HorologionApi.settings.open()` в `shared/ipc-types.ts` все согласованы.
- [ ] Если правил BrowserWindow — `loadWindowState` / `saveWindowState` / `scheduleWindowStateSave` в `electron/main.ts` сохранили window-state.json конвенцию.
- [ ] Тесты — на изолированной БД через `ARK_DB_PATH=.e2e/horologion-e2e.db`.

## Я правил usage-tracker (`services/usage-tracker`)

- [ ] `cargo test --manifest-path services\usage-tracker\Cargo.toml` — зелёный.
- [ ] Прямые ARK writes используют `ark_core::db` хелперы.
- [ ] `lan_sync.version_vector` обновляется после прямых писей.
- [ ] Default DB path остался `%APPDATA%\Kosmos\ark.db`.
- [ ] Тесты переопределяют DB path в `.tmp` / `.e2e` / OS temp.
- [ ] Tracker остаётся user-level, не Windows Service.

## Я правил `kosmos-visuals` (`packages/kosmos-visuals`)

- [ ] Не сломан public API (`index.ts` экспортирует те же имена).
- [ ] Если меняешь CSS-переменные в `theme/css-variables.css` — сразу отрази в `docs-site/.vitepress/theme/custom.css`.
- [ ] Token-файлы (`tokens/*.ts`) остаются source of truth для соответствующих переменных.

## Я правил документацию (`docs-site/`)

- [ ] `bun run docs:build` — собирается.
- [ ] Все внутренние ссылки рабочие.
- [ ] Русский язык, без английских заглушек.
- [ ] Не дублирую правила, лучше ссылка на канон-страницу.
- [ ] Обновил `MEMORY.md` или связанные `AGENTS.md` если факты изменились.

## Я делал substantial задачу через proof loop

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
