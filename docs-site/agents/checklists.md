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
