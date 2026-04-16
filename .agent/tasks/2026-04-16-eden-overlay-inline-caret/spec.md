# Task: Eden inline caret renders as overlay without breaking selection

## Goal
Restore a visible custom caret in the Eden ProseMirror editor without reintroducing drag-selection bugs. The existing inline-caret feature should stop using an in-flow widget anchor for the visible caret and instead render an overlay caret positioned from the editor selection.

## Component Map
- `apps/eden/ts/src/InlineCaret.ts`: ProseMirror/Tiptap inline caret behavior and positioning logic.
- `apps/eden/ts/src/Editor.css`: visual styling for the overlay caret and native caret handoff.
- `apps/eden/ts/tests/app.spec.ts`: regression coverage for visible editor caret behavior.

## Acceptance Criteria
- AC1: The Eden editor shows a visible custom caret when the editor is focused and the selection is collapsed.
- AC2: Drag-selection and non-collapsed selection continue to use native selection behavior.
- AC3: The visible custom caret is rendered without an in-flow widget anchor controlling the editor text flow.
- AC4: Existing app-level regression coverage is updated for the new overlay caret behavior.
- AC5: TypeScript verification passes for `apps/eden/ts`.
