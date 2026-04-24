# Spec: Arrancador dev startup stability and blank-screen regression

## Original task

User reports two dev-mode problems in `apps/arrancador`:

1. The app appears to crash when Alt+Tab switching away from it in dev mode.
2. The app window appears quickly, but a blank dark-blue screen remains visible for a long time before the UI renders.

## Scope

Investigate and fix the active Electron + Vue runtime for Arrancador so dev-mode startup is more stable and the user does not sit on a long blank shell before useful UI appears.

## Assumptions

- The reported Alt+Tab issue is specific to the Electron dev shell on Windows.
- The long blank screen is primarily a dev-startup problem, not a production build problem.
- We should prefer small, low-risk fixes over a large architectural startup rewrite.

## Acceptance Criteria

- AC1: The Windows dev shell no longer uses the most likely focus-sensitive crash path during startup, specifically the dev-only combination of early window showing and custom Windows titlebar handling.
- AC2: Cold startup in dev no longer eagerly imports all major route pages up front; the router loads page modules lazily.
- AC3: The user no longer sees only a blank dark background during renderer cold start; a visible startup placeholder is rendered until Vue mounts.
- AC4: Startup does not trigger duplicate initial game-library refreshes from both layout and the initial library page.
- AC5: The touched code passes fresh verification for typecheck/tests/builds relevant to Arrancador.

## Constraints

- Do not rewrite the entire Electron boot flow.
- Do not change unrelated application behavior outside startup/dev-shell stability.
- Keep the fix localized to Arrancador main process, router/startup shell, and immediate startup data flow.

## Non-goals

- Full startup telemetry or performance dashboarding.
- Broad refactors of the library page.
- Reworking production window chrome behavior outside the dev-specific stability fix.

## Verification plan

- Run `bun run typecheck` in `apps/arrancador`.
- Run `bun run test` in `apps/arrancador`.
- Run `bun run build:renderer` in `apps/arrancador`.
- Run `bun run build:main` in `apps/arrancador`.
- Inspect the resulting diff to confirm:
  - dev Windows chrome path is gated safely,
  - routes are lazy-loaded,
  - startup splash exists and is cleared on mount,
  - duplicate initial refresh is removed.
