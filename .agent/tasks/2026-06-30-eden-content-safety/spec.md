# Eden content safety

## Context

Eden is migrating from CodeMirror Markdown storage to TipTap/ProseMirror JSON. The critical requirement is data safety: editor content must not disappear after reload, refresh, navigation, migration, autosave, or live refresh.

## Scope

- Eden editor content serialization and migration.
- Eden store save, draft, refresh, hydrate, navigation, and live-refresh paths.
- Eden API/shim persistence paths used by renderer saves.

Out of scope:

- Sync protocol redesign.
- Removing temporary TipTap auto-migration.
- Visual polish unless it affects persistence.

## Acceptance Criteria

**AC1.** Existing Markdown-wrapper entries, TipTap entries, legacy ProseMirror entries, and invalid content are read without throwing; unsupported/invalid content is never destructively rewritten unless the user edit/save path explicitly saves a new draft.

**AC2.** Editor autosave, blur, and unmount paths persist the latest local body through `eden.handleSave`, even when optimistic drafts have already updated `currentEntry`.

**AC3.** A stale save, stale load, `refreshData`, `hydrateVaultData`, or live-refresh event cannot overwrite a newer local dirty draft for the current entry.

**AC4.** `listEntries`, `listAllEntries`, `loadEntry`, and `saveEntry` preserve `content_json` round-trip through the normal ARK/write-boundary path and do not replace real body content with summaries or empty defaults.

**AC5.** TipTap auto-migration scans all DB entries but only rewrites entries that are still in the legacy Markdown wrapper format; already-TipTap, deleted, collection, invalid, or unsupported entries are skipped.

**AC6.** Regression coverage exists for the edge cases fixed or confirmed during this task, and relevant Eden typecheck/unit/browser checks pass.
