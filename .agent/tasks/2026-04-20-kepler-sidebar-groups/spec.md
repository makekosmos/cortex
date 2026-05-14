# Kosmos Sidebar Groups

## Context

`Kosmos Visuals` sidebar currently renders flat sections:
- primary nav items
- one project list
- one secondary project list
- footer items

The shell lacks a reusable grouped presentation for related object rows. Eden needs this to present recent entries and object-type shortcuts as visually separated, collapsible blocks without forking sidebar rendering in the app layer.

## Acceptance criteria

### AC1. Generic grouped API
- `packages/kosmos-visuals/components/Sidebar.vue` supports grouped project sections as a reusable API.
- A group has a label, rows, and collapsed/expanded state.
- Existing non-group sidebar usage keeps working.

### AC2. Group visuals
- Groups render with a section header and a rounded surface/card behind the rows.
- Rows preserve the current hover and click behavior.
- Group headers visually match the provided compact desktop style.

### AC3. Collapse behavior
- Each group can be collapsed and expanded from its header.
- Collapse state is local to the sidebar instance.
- Hidden/collapsed sidebar behavior and resize behavior are not broken.

### AC4. Eden adoption
- Eden notes-mode sidebar uses grouped sections for `Недавние` and `Объекты`.
- Recent entries stay in the first group.
- Object types stay in the second group.

### AC5. Verification
- Build/type checks still pass for the touched package/app.
- Visual strings remain UTF-8 safe in touched source.
