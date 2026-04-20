# Eden Anytype-Like Object Types v1

## Status
Frozen before implementation.

## Goal
Build an Anytype-inspired object type experience in `Eden` on top of the Ark generic object layer:

- every object has the same note body editor
- object type differences live in the header above the note body
- built-in types `note_obj` and `game_obj` have code-backed field contracts
- the presentation of those fields is configurable from Settings
- Settings exposes a type editor that feels structurally similar to Anytype without copying its storage model

This task extends, but does not replace, `.agent/tasks/object-model-v1/spec.md`.

## Product Intent
We want the Anytype idea, not a literal port:

- keep Ark as the canonical backend and schema source
- keep a universal object model
- separate "what fields this type supports" from "how this type is shown"
- make the top of an object page feel like Anytype:
  - large title
  - optional description
  - compact featured properties above the shared note body

## Scope
- `packages/ark-core/rust`
- `apps/eden/ts/main`
- `apps/eden/ts/src`
- optional supporting updates in `packages/kepler-visuals` if a reusable header/property component is extracted

## Out Of Scope
- Full Anytype parity in dataview/grid/gallery/board filtering and sorting
- Backlinks UI beyond what already exists in `object_links`
- Porting Anytype React code directly into the repo
- Replacing the existing note body editor format

## Architecture Decision
Use two layers for object types:

### 1. Type Contract
Defines what fields exist for a type.

For built-in types this is code-backed and backend-aware.
Examples:
- `note_obj` supports `description`, `related_notes`
- `game_obj` supports `description`, `user_rating`, `play_status`, `genres`, `total_playtime_seconds`, etc.

### 2. Type Presentation
Defines how those fields are shown.

This is persisted in Ark and editable from Settings.
Examples:
- which fields are `featured`
- which fields are merely `visible`
- which fields are `hidden`
- field order
- header layout mode

Changing presentation must not change the backend contract for built-in types.

## Data Model

### Objects
Use the existing generic object model:

`objects`
- `id`
- `type_id`
- `title`
- `content_json`
- `props_json`
- `created_at`
- `updated_at`
- `deleted_at`

Rules:
- `title` stays top-level
- `content_json` stays the shared note body
- `props_json` stores type-specific properties

### Object Types
`object_types`
- `id`
- `name`
- `schema_json`
- `ui_schema_json`
- `created_at`
- `updated_at`
- `system_locked`

### Object Links
`object_links`
- `id`
- `source_object_id`
- `target_object_id`
- `link_type`
- `created_at`

## Type Schema

### `schema_json`
Defines supported fields.

Each field definition in v1 should support:
- `id`
- `label`
- `kind`
- `multiple`
- `required`
- `read_only`
- `system`
- `link_type`
- `allowed_object_types`

Supported kinds in v1:
- `text`
- `long_text`
- `number`
- `date`
- `boolean`
- `select`
- `multi_select`
- `image`
- `relation`

Nice-to-have later:
- `url`
- `email`
- `phone`
- `file`

### `ui_schema_json`
Defines presentation.

Required v1 keys:
- `featured_fields`
- `visible_fields`
- `hidden_fields`
- `read_only_fields`
- `field_order`
- `header_layout`
- `default_layout`
- `default_template_id`

Rules:
- `featured_fields` render above the editor body
- `visible_fields` are available in the normal object view
- `hidden_fields` stay out of the main flow
- `read_only_fields` render as non-editable
- built-in types may allow editing `ui_schema_json` while restricting contract edits

## Built-In Types

### `note_obj`
Contract fields:
- `title`
- `description`
- `related_notes`

Shared body:
- `content_json`

Hidden/system:
- `created_at`
- `updated_at`
- `deleted_at`

### `game_obj`
Contract fields:
- `title`
- `description`
- `user_rating`
- `play_status`
- `genres`
- `cover_image`
- `background_image`
- `related_notes`
- `exe_path`
- `save_path`
- `total_playtime_seconds`
- `last_played_at`
- `play_count`
- `save_exists`

Shared body:
- `content_json`

Hidden/system:
- `created_at`
- `updated_at`
- `deleted_at`
- `rawg_id`
- `exe_name`
- sync/source metadata fields needed by Arrancador and usage hydration

## UX Requirements

### Object Page
Every object page must render in this order:
1. `title`
2. `description` when present
3. featured properties
4. note body editor

The note body editor must remain the same across all object types.

### Header Layouts
Support two header property layouts in v1:
- `inline`
- `column`

### Settings
Add or evolve a Settings section for object types with:

1. Types list
- list built-in and user-created types
- built-in types clearly marked as system
- create-type entry point

2. Type editor
- general info
- field contract editor
- presentation editor
- live preview

3. Built-in type behavior
- allow presentation edits
- restrict destructive edits to code-backed fields
- clearly mark locked/system fields

## Visual Reference Constraints
The UI should take structure and interaction cues from Anytype:
- title separated from metadata
- featured properties as compact surfaces
- property stack above the note body
- preview in Settings

But implementation must remain idiomatic for current Vue/TypeScript code in this repo.

## Acceptance Criteria

- AC1: Ark stores type presentation separately from field contract, with `schema_json` and `ui_schema_json` able to express featured, visible, hidden, read-only, ordering, and header layout.
- AC2: `note_obj` and `game_obj` exist as built-in backend-aware types with code-backed field contracts and Ark-persisted presentation config.
- AC3: Eden object pages render title, optional description, featured properties, and the shared note body in a clear Anytype-like object-page hierarchy.
- AC4: Eden Settings provides a type editor for built-in and user-defined types, including a presentation editor with featured, visible, hidden, and ordering controls plus a preview surface.
- AC5: Built-in types support presentation customization without allowing backend-breaking contract edits.
- AC6: `related_notes` remains canonical in `object_links` and continues to work in the Anytype-like object page and type editor flows.
- AC7: `game_obj` still supports Arrancador and usage-tracker integration fields after the UI/presentation refactor.
- AC8: Type editing and object rendering flows remain valid UTF-8 with no mojibake in touched Russian strings.
- AC9: The refactor does not introduce obvious regressions in startup/editor interaction caused by new header/type-editor code.

## Implementation Stages

### Stage 1. Schema and Registry Alignment
Deliverables:
- finalize the type contract/presentation split in Ark/Eden
- ensure built-in type definitions are explicit and backend-aware
- align `note_obj` and `game_obj` field definitions with current integrations

Checks:
- type/model unit checks where available
- Rust build for Ark core binaries
- TypeScript typecheck for Eden

Pass condition:
- built-in types can be loaded with contract plus presentation metadata

### Stage 2. Object Header Refactor
Deliverables:
- replace the current mixed typed-note header with an object-page header
- render `title`, `description`, featured properties, then body
- support `inline` and `column` header layouts

Checks:
- Playwright visual and interaction smoke for object pages
- verify note body editor still mounts and edits normally
- verify scrolling stays scoped correctly to content surfaces

Pass condition:
- object page reads as a clean typed object rather than a form/grid

### Stage 3. Settings Type Editor
Deliverables:
- types list page
- type editor for general info, fields, presentation, preview
- lock behavior for built-in type contract fields

Checks:
- Playwright flow for opening Settings, selecting a type, changing presentation, saving, reloading, and observing the updated object page
- manual/CLI verification that built-in type contract edits are blocked where required

Pass condition:
- type presentation can be configured from Settings without breaking built-in types

### Stage 4. Built-In Type Presentation Customization
Deliverables:
- configurable visible/hidden/featured fields for `note_obj`
- configurable visible/hidden/featured fields for `game_obj`
- persisted field ordering

Checks:
- create a `note_obj`, hide/show fields, reload, verify persisted UI
- create or load a `game_obj`, hide/show usage/integration fields, reload, verify persisted UI
- verify read-only fields remain read-only in the editor surface

Pass condition:
- built-in types are customizable at the presentation layer only

### Stage 5. Integration Safety Pass
Deliverables:
- confirm Arrancador/usage field hydration still maps cleanly into `game_obj`
- ensure object page and Settings changes do not break existing object save flows

Checks:
- integration smoke tests where available
- targeted CLI verification of read/write flows
- manual inspection of persisted Ark object rows if needed

Pass condition:
- `game_obj` remains a stable integration target

### Stage 6. Quality and Regression Pass
Deliverables:
- UTF-8 and mojibake review for all touched UI strings
- startup and interaction sanity checks
- evidence package for the completed implementation

Checks:
- Playwright smoke on the main paths touched by the change
- optional Lighthouse CLI or equivalent performance snapshot for editor/settings pages if a browser-run harness is practical
- static search for common mojibake fragments in touched files and built output where practical

Pass condition:
- no known regressions, no mojibake, no unresolved acceptance criteria

## Verification Plan

### Command-Level Verification
At minimum, rerun as applicable after implementation:
- Ark Rust build
- Eden TypeScript typecheck
- relevant unit/integration tests
- Playwright CLI scenarios for object page and Settings

### Playwright Coverage
Required scenarios:
- open a `note_obj` page and verify header hierarchy
- open a `game_obj` page and verify header hierarchy
- change type presentation in Settings and confirm live effect after reload
- verify hidden fields do not appear in the main object header
- verify read-only fields cannot be edited
- verify related-note links still work

### Performance / Responsiveness
Where practical:
- measure initial editor/settings render with Playwright traces
- use Lighthouse CLI only if there is a stable browser-accessible target for the changed pages
- compare before/after startup or interaction timings if a reproducible harness exists

Performance checks are advisory unless a clear regression is introduced.

### UTF-8 / Mojibake Verification
Required for touched UI files:
- inspect Russian strings directly in source
- run a targeted search for mojibake patterns such as `Р`, `Ð`, `Ñ`, replacement glyphs, or obviously broken mixed encodings
- manually open the changed UI and verify rendered Russian text

## Evidence Requirements
When implementation starts and progresses, record:
- `evidence.md`
- `evidence.json`
- raw Playwright artifacts
- raw CLI outputs for build/typecheck/test runs
- screenshots where visual checks matter

If any stage fails verification:
- write `problems.md`
- apply the smallest safe fix
- rerun the failed checks

## Constraints
- Do not regress current Ark-backed object persistence
- Do not break Arrancador game-object synchronization
- Do not replace the shared note body editor
- Do not silently mutate built-in type contracts from Settings
- Watch UTF-8 carefully and explicitly check for mojibake in every touched UI surface

## Definition Of Done
This task is done only when:
- every acceptance criterion is verified as PASS
- the type editor and object page work for `note_obj` and `game_obj`
- presentation customization persists correctly
- integration safety checks pass
- UTF-8/mojibake checks pass
- evidence artifacts are present and current
