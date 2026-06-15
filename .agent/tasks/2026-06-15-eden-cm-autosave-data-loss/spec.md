# 2026-06-15 — Eden CM autosave data loss

## Context

User reported that text typed into an Eden note disappeared after leaving the note and returning.
The affected surface is the CodeMirror markdown editor save path.

## Scope

In scope:

- `products/eden/src/editor-cm/CmEditor.vue` dirty/persisted-state handling.
- Regression tests for optimistic parent draft updates and editor persistence.
- Postmortem entry in `docs-site/agents/postmortems.md`.

Out of scope:

- ARK schema or sync protocol changes.
- Direct database migration or data recovery.
- Unrelated stale Vim/preferences tests.

## Acceptance Criteria

**AC1.** When the user edits a note body, the editor still calls `onSave` after the debounce even if the parent has already applied `entryDraftChange` optimistically.

**AC2.** Leaving/unmounting an edited note before the debounce fires must still flush the latest body text to `onSave`.

**AC3.** A regression test covers the optimistic parent draft scenario that previously skipped persistence.

**AC4.** The fix does not introduce direct SQL writes and continues to use Eden's existing `onSave` / ARK save boundary.

**AC5.** Postmortem documents the root cause and prevention for optimistic draft state masking unsaved editor changes.
