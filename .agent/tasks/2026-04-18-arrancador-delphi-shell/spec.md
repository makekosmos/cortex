# Task Spec - Arrancador shell aligned to Delphi desktop chrome

## Original task
`Изучи как в delphi/ts сделан сайдбар и татйтл бар, и сделай ровно также в arrancador`

## Goal
Перевести desktop shell `apps/arrancador` на ту же композицию и поведение, что используются в `apps/delphi/ts`: titlebar с кнопкой панели и history controls, скрываемый desktop sidebar с persisted config, и content surface с тем же chrome-контрактом.

## Relevant context
- `apps/delphi/ts/src/App.vue` использует shared shell composition: `DesktopChrome`, `TitlebarHistoryControls`, слот `sidebar`, и `DesktopContentSurface`.
- `apps/delphi/ts/src/components/SideBar.vue` опирается на persisted sidebar config с `hidden`, а не на простое collapse-состояние.
- `apps/arrancador` уже имеет custom Electron window contract и React titlebar/sidebar, но их поведение не совпадает с Delphi-shell.
- `arrancador` — React renderer, поэтому прямой импорт Vue-компонентов из `@kepler/visuals` не является минимальным решением; нужен React-перенос того же shell contract.

## Acceptance Criteria
- AC1: Desktop titlebar `arrancador` повторяет Delphi-shell composition: leading controls содержат кнопку sidebar toggle и history back/forward controls вместо текущего centered titlebar layout.
- AC2: Desktop sidebar `arrancador` повторяет Delphi-shell behavior model: persisted config keyed отдельно от старого collapse-state, поддерживает `hidden` mode как основной desktop toggle, и визуально/структурно соответствует Delphi sidebar shell.
- AC3: Main layout `arrancador` повторяет Delphi-shell composition: desktop chrome wraps sidebar + content surface, а content surface adjusts border/radius based on sidebar visibility while mobile navigation continues to work.
- AC4: Existing window chrome IPC/native window setup continues to support the Delphi-like shell without regression on Windows/macOS.
- AC5: Regression coverage is updated for the new titlebar/sidebar/layout behavior, and fresh verification on current codebase passes.

## Constraints
- Keep edits focused on `apps/arrancador` and task artifacts.
- Do not modify `apps/delphi/ts` or unrelated dirty files.
- Do not import Vue SFCs into `arrancador`.
- Preserve working mobile menu behavior.

## Non-goals
- No redesign of feature pages beyond the outer shell.
- No route-level feature changes in `arrancador`.
- No migration of `arrancador` to Vue or direct adoption of the Delphi store/router layer.

## Assumptions
- “Ровно также” applies to shell composition and interaction model, not to Delphi-specific task/project domain items.
- Arrancador-specific navigation items remain, but the shell container/controls should behave like Delphi.

## Component map
- `src/components/AppTitlebar.tsx`: Delphi-like leading controls and renderer window controls.
- `src/components/Sidebar.tsx`: Delphi-like persisted hidden/resizable desktop sidebar plus Arrancador nav content.
- `src/pages/Layout.tsx`: desktop shell composition and mobile fallback behavior.
- `src/index.css`: supporting chrome/surface/sidebar styles matching the Delphi shell contract.

## Verification plan
- `bun run typecheck`
- `bun run test`
- `bun run build:renderer`
- `bun run build:main`
- `bun run build:preload`
