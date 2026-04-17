# Task Spec: Restore Sidebar hide shortcut on `Ctrl+B` / `Ctrl+И`

## Original Task

у сайдбара пропало скрытие с прошлых версий на ctrl + b / ctrl + и

## Scope

Restore the shared sidebar toggle shortcut behavior so the current `@kepler/visuals` `Sidebar` correctly toggles on the old hide shortcut in both Latin and Russian keyboard layouts.

## Assumptions

- The regression is in the shared keyboard shortcut matching for `packages/kepler-visuals/components/Sidebar.vue`.
- The desired behavior is that the sidebar toggle continues to work for the physical `B` key and for the localized Russian key value `и`.

## Constraints

- Make the smallest safe fix in shared sidebar code.
- Preserve existing `toggleShortcut` prop contracts for current consumers.
- Do not break other shortcut combinations already supported by `Sidebar`.

## Non-goals

- Redesigning shortcut configuration across apps.
- Adding a new global shortcut system.
- Refactoring unrelated sidebar state flow.

## Acceptance Criteria

- AC1: `Sidebar.vue` matches `Ctrl+B` / `Meta+B` reliably using both physical key codes and localized key values.
- AC2: Existing consumers that pass `toggle-shortcut="meta+b|ctrl+b"` work without needing app-specific workaround strings.
- AC3: Current shared sidebar code and Delphi consumer typecheck successfully after the fix.

## Verification Plan

1. Inspect current shortcut matching in `Sidebar.vue`.
2. Apply the smallest safe normalization fix for localized key aliases.
3. Run focused TypeScript checks for `apps/delphi/ts` and `apps/dashboard`.
