# 2026-07-11 daedalus-mvp

## Context

Kosmos не имеет first-party интерфейса для управления долгоживущими Codex-сессиями. Daedalus добавляет Vue extension, а существующий `kepler-backend` становится владельцем Codex app-server процессов, Git worktree и локальной истории; renderer общается с ним только через типизированный `@kosmos/ark` RPC.

## Scope

В задаче:

- Добавить `products/daedalus` как Vue 3/Pinia extension с русским UI, timeline, composer, approvals и Changes panel.
- Добавить lazy `agents.*` service в `platform/runtime`, отдельную SQLite БД и один `codex app-server` на активную сессию.
- Создавать из существующего Git-репозитория изолированную branch/worktree на каждую сессию и безопасно строить diff относительно base commit.
- Добавить typed API `client.agents` в `@kosmos/ark` и flat `agents_event` envelope.
- Добавить permission-gated `window.kepler.dialogs.pickDirectory()` и открытие worktree во внешнем редакторе.
- Восстанавливать persisted sessions и approvals после перезапуска; неизвестные app-server события не должны завершать сессию.
- Добавить unit/integration/contract/headless проверки, архитектурную документацию и визуальную верификацию.

Не в задаче:

- Android/Kotlin client, Iroh/relay/remote access и изменение `iroh-spike`.
- Claude/ACP, собственный agent loop, MCP/skills manager, subagent orchestration или terminal emulator.
- Импорт чужих Codex threads, встроенный editor, review comments, notifications, commit/push/merge/PR и cloud sandbox.

## Constraints

- `platform/runtime` остаётся единственным backend-процессом; новый daemon или extension sidecar запрещён.
- Codex интеграция использует только JSONL app-server protocol и установленный CLI/config/credentials.
- Daedalus хранит данные в `<data-dir>/extensions-data/daedalus/daedalus.db`, не в ARK sync tables.
- Renderer не получает произвольный filesystem access и не открывает SQLite.
- Production Rust locks восстанавливаются после poison; event wire format плоский.
- Все строки UI русские, визуальные значения берутся из `@kosmos/visuals`.

## Component Map

- `products/daedalus/src/App.vue`: тонкая композиция shell и выбранного экрана.
- `products/daedalus/src/stores/agents.ts`: snapshot, ordered event reduction, selection и actions.
- `products/daedalus/src/services/agentsClient.ts`: единственная точка typed RPC/event доступа.
- `products/daedalus/src/components/DaedalusSidebar.vue`: проекты, сессии, attention/status.
- `products/daedalus/src/components/StartScreen.vue`: добавление проекта и создание задачи.
- `products/daedalus/src/components/SessionTimeline.vue`: virtualized timeline и карточки событий.
- `products/daedalus/src/components/SessionComposer.vue`: mode/model, send/follow-up/interrupt.
- `products/daedalus/src/components/ChangesPanel.vue`: дерево файлов и unified diff.
- `platform/runtime/src/agents/*`: persistence, Git/worktree/diff, app-server codec/session actor и RPC service.
- `core/ark/packages/ark/src/ark-client-agents.types.ts`: публичные wire/client types.

## Acceptance Criteria

AC1. Extension contract

- `products/daedalus/manifest.json` объявляет Vue extension `0.1.0`, порт 9917, окно 1280×800/min 900×600, `windowEffect: none`, `daedalus:open` и только permissions `agents.read`, `agents.control`, `dialogs.directory`.
- Extension discovery/contract test проходит.

AC2. Projects and worktrees

- Добавление принимает только существующий Git repository и сохраняется в отдельной Daedalus DB.
- Создание сессии фиксирует текущий HEAD, создаёт `codex/<slug>-<short-id>` и worktree в Daedalus data dir.
- Tests покрывают dirty base, параллельные worktree и diff для committed/uncommitted/untracked файлов.
- Archive скрывает session и сохраняет worktree; отдельное подтверждаемое удаление работает только для archived, inactive и clean worktree без force.

AC3. Codex lifecycle

- На активную сессию запускается отдельный `codex app-server`; initialize/thread start-or-resume/turn start, follow-up, interrupt и model/list используют JSONL.
- Режимы `default`, `auto-review`, `full-access` маппятся в app-server параметры; full-access требует явного подтверждения UI.
- Fake app-server tests покрывают streaming, approvals, interrupt, две параллельные сессии и восстановление.

AC4. Persistence and events

- SQLite сохраняет projects, sessions, normalized timeline, approvals и worktree metadata.
- Timeline поддерживает cursor pagination, command output ограничен 1 MiB с `truncated: true`, pending approvals восстанавливаются.
- Renderer получает snapshot и затем упорядоченные flat `agents_event` с монотонным `seq`; unknown events только логируются.

AC5. Typed and safe desktop APIs

- `@kosmos/ark` экспортирует typed `client.agents` для перечисленных project/session/approval/diff/model/editor операций и `onEvent`.
- `window.kepler.dialogs.pickDirectory()` permission-gated и возвращает только выбранный directory path.
- `bun run --cwd core/ark/packages/ark typecheck` и desktop typecheck/build проходят.

AC6. UI behavior

- Стартовый экран показывает проекты/активные сессии/status/attention; session screen имеет sidebar, virtualized timeline, composer и сворачиваемый Changes panel.
- Timeline отображает messages, reasoning summary, plan, command, file change, error, request_user_input и approval cards.
- Changes показывает дерево и diff; editor picker обнаруживает code/cursor/windsurf с Explorer fallback.

AC7. Verification

- Focused Rust unit/integration tests, Daedalus typecheck/unit/build, `platform/desktop` typecheck/build, extension contract и `bun run ark:guard:writes` проходят.
- Headless Playwright покрывает создание проекта/session, streaming, approval, parallel session switch, Changes и reopen без остановки backend.
- Visual verification подтверждает 1280×800 и minimum viewport; real Codex smoke остаётся opt-in и не запускается в обычном CI.

AC8. Documentation

- Source docs описывают Daedalus runtime boundary, storage, recovery и deferred Android protocol; `bun run docs:sync` и `bun run docs:check` проходят.

## Verification commands

- `cargo test --manifest-path platform/runtime/Cargo.toml agents`
- `bun run --cwd core/ark/packages/ark typecheck`
- `bun run --cwd products/daedalus typecheck`
- `bun run --cwd products/daedalus test`
- `bun run --cwd products/daedalus build`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build`
- `bun run ark:guard:writes`
- Headless Playwright command from `docs-site/agents/testing.md` with isolated `KOSMOS_DATA_DIR`.
- `bun run docs:sync`
- `bun run docs:check`

## Out of scope decisions

- App-server protocol is experimental, поэтому adapter/codec остаётся изолированным внутри `platform/runtime/src/agents`.
- Archive сохраняет worktree. Его удаление реализовано отдельной подтверждаемой операцией и отказывается удалять active или dirty worktree.
