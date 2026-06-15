# 2026-06-15 — Eden settings and trash cleanup

## Context

User wants Eden settings to behave like a separate settings surface instead of a note/object workspace, remove unused object-type management screens/import controls, and make deletion/trash semantics coherent.

## Scope

In scope:

- Eden settings chrome/titlebar/sidebar behavior in `products/eden/src/App.vue` and `components/sidebar/EdenSidebar.vue`.
- Removal of object-type management screens and the related type-collection/custom page surface from Eden navigation.
- Removal of single-object Markdown import from general settings while keeping export/vault flows unless directly required by cleanup.
- Trash behavior for Eden entries: normal delete sends entries to trash; trash supports restore and permanent delete with explicit confirmation.
- Sidebar delete refresh bug where a deleted entry remains visible until a second delete attempt.

Out of scope:

- ARK schema redesign or sync protocol changes.
- Data recovery for entries already deleted before this change.
- Removing system object types themselves from ARK.
- Reworking the whole settings design system beyond requested controls and sidebar behavior.

## Acceptance Criteria

**AC1.** Opening Eden settings shows the settings surface without note-workspace titlebar controls: no sidebar hide/show, settings, search, create-note, back-to-notes, or reader/writer toggle controls.

**AC2.** While settings is active, Eden disables maximize for the extension window when the preload API supports it, leaving only minimize/close native controls available; leaving settings restores maximizable behavior.

**AC3.** Settings sidebar navigation highlights the selected settings page with a background color and cannot be resized in settings.

**AC4.** Object-type management screens are removed from user navigation and app routing: no `object-types` screen, no `type-collection` screen, no objects modal/create-type flow.

**AC5.** General settings no longer contains the single-object Markdown import button/flow.

**AC6.** Normal entry deletion removes the entry from active sidebar/list state immediately and keeps it restorable through Trash.

**AC7.** Trash lists soft-deleted entries, restore returns them to normal lists, and permanent deletion actions require explicit confirmation.

**AC8.** All Eden data writes continue through existing `window.api` / ARK operations; no direct SQL writes are introduced.

**AC9.** Relevant component/unit tests cover settings sidebar cleanup and delete/trash behavior.
