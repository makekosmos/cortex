# Task Spec — Eden backgrounded E2E window

## Source of truth
- `.omx/context/eden-e2e-background-window-20260415T000000Z.md`
- `.omx/plans/prd-eden-e2e-background-window.md`
- `.omx/plans/test-spec-eden-e2e-background-window.md`

## Goal
Keep Eden hidden during Playwright/E2E background launches on macOS without breaking test stability.

## Acceptance Criteria
- AC1: Background launch path no longer reveals the window.
- AC2: Normal dev launch behavior remains intact.
- AC3: `bun run lint` passes.
- AC4: `bunx tsc --noEmit -p tsconfig.json` passes.
- AC5: `bun run build` passes.
- AC6: `bun run test:e2e` passes.
