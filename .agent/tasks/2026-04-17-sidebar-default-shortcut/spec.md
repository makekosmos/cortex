# Task Spec: Make sidebar hide shortcut work everywhere by default

## Original Task

в eden не работает сркытие раскрытие сайдбара на ctrl + b / и. должно быть одинаково везде без доп настроек

## Scope

Restore the shared sidebar toggle shortcut as a built-in default in `@kosmos/visuals` so every app gets the same `Ctrl/Meta + B` and localized-layout behavior without needing to pass `toggleShortcut`.

## Assumptions

- The current regression is that some consumers, especially Eden, do not pass `toggle-shortcut`, so the shared sidebar never registers a keyboard toggle.
- The shared `Sidebar.vue` is the correct single source of truth for this default behavior.

## Constraints

- Make the smallest safe change in shared sidebar code.
- Preserve the ability for consumers to override `toggleShortcut`.
- Do not add app-specific workaround logic in Eden.

## Non-goals

- Refactoring Eden layout state management.
- Adding a separate global shortcut manager.
- Changing sidebar persistence behavior.

## Acceptance Criteria

- AC1: `packages/kosmos-visuals/components/Sidebar.vue` uses a default toggle shortcut so keyboard toggling works even when consumers omit `toggleShortcut`.
- AC2: Eden keeps using the shared sidebar without extra local shortcut props and receives the same behavior as Delphi/dashboard.
- AC3: Focused type checks pass for current consumers after the change.

## Component Map

- `Sidebar.vue`: owns default keyboard shortcut behavior for the shared sidebar.
- `EdenSidebar.vue`: consumer wrapper; should not require any new workaround prop for this behavior.

## Verification Plan

1. Confirm Eden consumer omits `toggleShortcut` while Delphi/dashboard currently rely on shared sidebar.
2. Add the shared default shortcut in `Sidebar.vue`.
3. Run focused type checks for Eden, Delphi, and dashboard consumers.
