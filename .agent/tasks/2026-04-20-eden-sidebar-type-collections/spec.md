# Eden Sidebar Type Collections

## Context

Eden already has:
- shared `Kosmos Visuals` sidebar shell
- note type editor and built-in object types
- recent notes in the sidebar
- settings and object-type editing screens

The missing pieces are:
- typed icons in sidebar rows
- an `Объекты` section that leads to collection pages per type
- pluralized collection labels for object types
- a collection page for each object type with create/edit actions

This task must preserve the current shared shell and keep note type metadata backward-compatible by storing collection-specific presentation in `ui_schema_json`.

## Component map

- `App.vue`: shell composition and screen routing
- `store/eden.ts`: object/type navigation state and type-aware entry creation
- `EdenSidebar.vue`: sidebar sections, note rows, object type rows
- `typedNotes.ts`: note type UI schema parsing helpers
- `object-types/shared.ts` + `ObjectTypeIdentitySection.vue`: plural type editing
- `TypeObjectsView.vue` (new): collection page for one object type

## Acceptance criteria

### AC1. Type plural metadata
- Each note type supports a collection/plural label stored in `ui_schema_json`.
- Built-in types provide sensible plural labels.
- The type editor can read and save this value.
- Existing types without this value fall back to a generated plural label.

### AC2. Sidebar note rows
- Recent/all note rows display the type icon to the left of the entry title.
- These note-row icons are visually neutral, not type-colored.
- Text stays left-aligned and truncates cleanly.

### AC3. Sidebar objects section
- Notes mode sidebar includes a separate `Объекты` section.
- This section lists object types, using their type icon and type color.
- Clicking an object type opens a dedicated collection page for that type.

### AC4. Type collection page
- Eden has a dedicated page for a selected object type.
- The page title uses the type plural label.
- The page lists entries of that type.
- The page exposes actions to create a new object of that type and open type editing.

### AC5. Navigation and persistence
- Creating from a type collection page opens a new entry with that type already assigned.
- Opening type editing from the collection page lands on the corresponding type in settings.
- Returning from settings/object-type screens still works.
- Existing note/settings/object-type screens continue to render.

### AC6. Verification
- TypeScript/build checks pass for the touched app/package.
- Verification artifacts are written under this task directory.
- Touched Russian UI strings are checked for UTF-8 safety / no mojibake in source edits.
