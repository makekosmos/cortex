# Evidence - Arrancador spotlight titlebar removal and keyboard scroll sync

## Scope

- `apps/arrancador/src/components/AppTitlebar.tsx`
- `apps/arrancador/src/components/Spotlight.tsx`
- `apps/arrancador/src/pages/Layout.tsx`
- `apps/arrancador/src/test/app-titlebar.test.tsx`
- `apps/arrancador/src/test/layout.test.tsx`
- `apps/arrancador/src/test/spotlight.test.tsx`
- `.agent/tasks/2026-04-19-arrancador-spotlight-titlebar-scroll/spec.md`

## What changed

- Removed desktop titlebar search rendering from `AppTitlebar`; the center slot is now empty chrome only, and desktop search stays outside the titlebar.
- Stopped `Layout` from passing any titlebar search-trigger state when the sidebar is hidden.
- Added spotlight active-option refs plus `scrollIntoView({ block: "nearest" })` so keyboard navigation keeps the selected result visible.
- Added/updated tests to cover:
  - titlebar without search
  - hidden-sidebar layout without desktop titlebar search
  - spotlight reopen/reset behavior
  - spotlight close-on-route-change behavior
  - spotlight keyboard navigation scrolling behavior

## Acceptance criteria

- `AC1` PASS: desktop titlebar no longer renders search when the sidebar is hidden.
- `AC2` PASS: spotlight arrow-key navigation scrolls the active result into view.
- `AC3` PASS: relevant Arrancador tests cover the updated titlebar and spotlight behavior.
- `AC4` PASS: `bun run typecheck` and `bun run test` pass in `apps/arrancador`.

## Verification

### Fresh commands

- `bun run typecheck` PASS
- `bun run test` PASS (`31 passed` files, `143 passed | 2 expected fail` tests)

### Raw artifacts

- `.agent/tasks/2026-04-19-arrancador-spotlight-titlebar-scroll/typecheck.clean.log`
- `.agent/tasks/2026-04-19-arrancador-spotlight-titlebar-scroll/test.clean.log`

## Notes

- `vitest` output includes expected provider-guard stack traces from tests that intentionally assert hook misuse; the run still passes.
- A read-only subagent review found no remaining desktop titlebar path that renders spotlight search. Remaining search triggers are in the sidebar and the mobile top bar only.
