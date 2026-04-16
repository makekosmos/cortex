# Task: Eden editor falls back to native caret

## Goal
Restore standard native text caret and drag-selection behavior in the Eden ProseMirror editor, while keeping the existing inline-caret implementation in the codebase. The inline-caret code should remain present, but the visible/active editor behavior should use the native caret.

## Component Map
- `apps/eden/ts/src/Editor.css`: active ProseMirror caret behavior and inline-caret visuals.
- `apps/eden/ts/src/InlineCaret.ts`: existing inline-caret implementation retained in code, not deleted.

## Acceptance Criteria
- AC1: The Eden editor uses the native browser caret in the ProseMirror surface again.
- AC2: Drag-selection in the editor is no longer blocked by the visible inline-caret styling layer.
- AC3: The inline-caret implementation remains in the codebase and is not deleted.
- AC4: TypeScript verification passes for `apps/eden/ts`.
