# Task: Revert Eden overlay inline caret experiment

## Goal
Roll back the recent overlay-based inline caret experiment in Eden and return to the previous safer state: native browser caret remains active in the ProseMirror editor, while the older inline-caret implementation stays in the codebase without the new screen-position overlay logic.

## Component Map
- `apps/eden/ts/src/InlineCaret.ts`: revert from overlay positioning back to the prior widget-based implementation.
- `apps/eden/ts/src/Editor.css`: restore the previous native-caret fallback styling.
- `apps/eden/ts/tests/app.spec.ts`: revert overlay-specific assertions to the prior expectations.

## Acceptance Criteria
- AC1: Eden no longer uses the overlay/screen-position inline caret implementation.
- AC2: The editor returns to the previous native-caret fallback state.
- AC3: The historical inline-caret implementation remains in the codebase and is not deleted.
- AC4: TypeScript verification passes for `apps/eden/ts`.
