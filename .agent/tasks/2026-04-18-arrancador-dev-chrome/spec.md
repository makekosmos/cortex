# Task Spec - Arrancador dev startup and shared chrome pass

## Original task
`Изучи arrancador, чет сейчас через bun run dev Запускаю и прям очень долго грузит. Проверь что там, используй sub agents. Также применю сюда sidebar + titlebar из kepler-visuals`

## Goal
Ускорить локальный запуск `apps/arrancador` через `bun run dev` за счет устранения лишней работы на старте и перевести оболочку окна `arrancador` на визуальный паттерн `kepler-visuals` (titlebar + sidebar) без прямого смешения Vue-компонентов с React-рендерером.

## Relevant context
- `apps/arrancador/package.json` currently runs a native rebuild on every `bun run dev` via `predev`.
- `apps/arrancador/scripts/dev.ts` already starts Vite and Bun watchers quickly once predev is done.
- `packages/kepler-visuals` exports Vue components, so React `arrancador` needs a local shell implementation that matches the shared chrome contract rather than importing `.vue` files.
- `packages/kepler-visuals/components/Sidebar.vue` and `Titlebar.vue` are already dirty in the workspace; avoid unnecessary edits there.

## Acceptance Criteria
- AC1: `apps/arrancador` no longer forces a native `better-sqlite3` rebuild on every `bun run dev`; `predev` skips rebuild when the local native artifact cache is still valid and rebuilds when the cache is missing or stale.
- AC2: The rebuild-skip logic is deterministic and keyed off explicit inputs relevant to native compatibility (at minimum Electron version, `better-sqlite3` version, platform, architecture, and artifact presence), while the explicit manual rebuild path remains available.
- AC3: The desktop `arrancador` shell uses a kepler-visuals-style chrome: a dedicated draggable titlebar area plus a sidebar surface integrated into the main layout on desktop, while mobile navigation still works.
- AC4: Electron main/preload/renderer expose the minimal window chrome contract needed for the new titlebar shell (platform detection and window controls) and configure native window options so the custom titlebar is usable on macOS and Windows.
- AC5: Regression coverage proves the new dev-cache behavior and the new shell behavior, and current verification for `arrancador` passes on the modified codebase.

## Constraints
- Keep edits focused on `apps/arrancador` and task artifacts unless a minimal supporting change is strictly required.
- Do not revert unrelated dirty workspace changes.
- Do not introduce a Vue runtime dependency into `arrancador`.
- Preserve the existing explicit `rebuild:native` script for manual recovery.

## Non-goals
- No redesign of feature pages beyond the outer app shell.
- No migration of `arrancador` from React to Vue.
- No broad refactor of Electron backend services unrelated to window chrome or dev-start performance.
- No claim that cold native rebuild becomes free; only repeated unchanged dev starts should skip unnecessary rebuild work.

## Assumptions
- The user's main complaint is repeated local dev startup latency, not packaged app launch time.
- A React port of the shared chrome is acceptable as long as it follows the same visual and behavioral contract from `kepler-visuals`.
- Fresh verification can rely on typecheck/tests/build commands and targeted script checks; full interactive Electron startup timing is not required for every acceptance criterion.

## Component map
- `scripts/rebuild-native-if-needed.ts`: decides whether `predev` should rebuild `better-sqlite3` or reuse a valid cached native artifact stamp.
- `src/components/AppTitlebar.tsx`: renders the draggable desktop titlebar, history/window controls, and shell-level actions.
- `src/components/Sidebar.tsx`: updated to visually align with the `kepler-visuals` sidebar contract while preserving current navigation content.
- `src/pages/Layout.tsx`: composes titlebar, sidebar, mobile nav behavior, and routed content into the new shell.
- `electron/main/windows.ts` + preload/IPC types/API: provide native window configuration and control hooks used by the titlebar.

## Verification plan
- `bun run test`
- `bun run typecheck`
- `bun run build:renderer`
- `bun run build:main`
- `bun run build:preload`
- Targeted script checks for rebuild cache behavior (skip vs rebuild decision paths)
