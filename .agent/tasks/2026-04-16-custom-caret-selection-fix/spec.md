# Task: Custom caret preserves native text selection behavior

## Goal

Fix the shared `@kosmos/visuals` custom caret so text fields keep normal native text selection behavior. The custom caret must remain enabled for collapsed caret states, but when the user has an expanded text selection the component should stop forcing transparent native caret mode and allow standard selection UX.

## Component Map

- `packages/kosmos-visuals/components/CustomCaret.vue`: shared custom caret implementation and native caret toggling.
- `apps/eden/ts/tests/app.spec.ts`: regression coverage for input selection behavior while the shared custom caret is mounted.

## Acceptance Criteria

- AC1: The shared custom caret still renders for focused collapsed caret states in supported inputs/contenteditables.
- AC2: When the active target has a non-collapsed text selection, the custom caret hides and the native caret mode is restored so standard text selection interactions work.
- AC3: When the selection collapses again, the shared custom caret takes over again without requiring blur/refocus.
- AC4: A regression test covers the selection-mode handoff while the shared custom caret is active.
- AC5: TypeScript verification passes for the touched app/package code.
