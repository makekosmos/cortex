# Task Spec - Eden startup hydration perf

## Goal
Reduce perceived startup latency in `apps/eden/ts` so the shell can render before full vault hydration completes, then document the verification status of `bun run dev` and Playwright in the current environment.

## Scope
- `apps/eden/ts/src/store/eden.ts`
- `apps/eden/ts/src/App.vue`
- `apps/eden/ts/src/App.css`

## Acceptance Criteria
- AC1: Eden startup no longer blocks the initial shell render on `listEntries()` and `listNoteTypes()`.
- AC2: While vault data is hydrating in the background, the main surface shows an explicit non-blocking loading state instead of keeping the full app behind the bootstrap loader.
- AC3: Eden still passes TypeScript and production build verification after the startup-flow change.
- AC4: Current `bun run dev` and Playwright diagnostic results are captured in raw evidence, including any environment blocker that prevents Electron or Playwright worker startup.
