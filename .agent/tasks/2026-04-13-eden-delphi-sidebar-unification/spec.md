# Task: Unify Eden main sidebar and macOS titlebar behavior with Delphi

## Context

`apps/delphi/ts` uses a single primary sidebar pattern built on shared Kosmos sidebar primitives and a macOS window configuration with `titleBarStyle: hiddenInset` plus inset traffic lights. `apps/eden/ts` still uses a custom Anytype-like main sidebar (`WidgetSidebar.vue`) and a separate renderer titlebar overlay.

The goal is to make Eden's main sidebar and macOS window/header behavior feel aligned with Delphi while preserving Eden-specific data flows and keeping the existing spaces content screens intact.

## Constraints

- Do not introduce `vue-router` into Eden just to reuse Delphi's exact sidebar component.
- Keep the existing spaces content pages and active-space semantics.
- Do not remove the separate vault sidebar in Eden.
- Preserve existing test ids where feasible so current E2E flows keep working.
- Make the smallest defensible diff that aligns behavior and structure.

## Acceptance Criteria

- AC1: Eden replaces the current custom main widget sidebar shell with a Delphi-like primary sidebar structure based on shared sidebar primitives and a flat nav model for main destinations.
- AC2: Eden keeps current space flows working: selecting `my-space`, `all-objects`, `all-notes`, `all-properties`, `diary`, opening settings, search, and creating a note still work from the new sidebar.
- AC3: Eden keeps the current vault sidebar and `SpacesView` behavior unchanged except for the integration needed to coexist with the new main sidebar.
- AC4: Eden macOS window configuration and renderer titlebar behavior are updated to match Delphi's window-controls approach more closely, including inset traffic lights and drag-region handling that does not block sidebar interactions.
- AC5: Eden build and E2E checks pass against the current codebase, and proof artifacts record the verification status.
