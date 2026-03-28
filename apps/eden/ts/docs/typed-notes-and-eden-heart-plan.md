# Typed Notes and eden-heart Plan

This document captures the recommended architecture direction for Eden based on the current codebase.

## Product shape

Eden should stay **document-first**.

Each note has two layers:

1. **Typed header**
   - visual, schema-driven, configurable in the app
   - defines note type and structured properties
   - affects rendering, filtering, sorting, and future search
2. **Body**
   - regular TipTap document
   - same editor for all note types
   - stored as the existing `content_json` + markdown projection

This is intentionally **not** a full Anytype-style object graph in phase 1.

## Why this fits the current app

The repo is already shaped around a single document entity:

- `electron/store.ts` stores one `Entry` row per note
- `src/Editor.tsx` already separates header UI from body editor
- `src/components/sidebar/Sidebar.tsx` renders one item per note
- `electron/search.ts` and `electron/main.ts` currently search title + markdown body

That means typed headers can be added as an additive layer without replacing the editor model.

## Core architecture decision

Use a **typed note model** now, and treat `eden-heart` as **phase 2+ domain core extraction**.

Do **not** attempt all of this in one pass:

- user-defined types
- visual header builder
- Zod validation
- Rust core
- Tantivy search
- sync
- encryption

That would create too many moving parts at once.

## Phase 1: Typed note foundation in the current Electron app

### Goal

Allow notes to have a type, structured header properties, and a visual header layout, while keeping the body editor unchanged.

### Data model

Current `entries` table should be extended with:

- `type_id TEXT NULL`
- `header_layout TEXT NULL`
- `header_props_json TEXT NOT NULL DEFAULT '{}'`
- `schema_version INTEGER NOT NULL DEFAULT 1`

And add a new table:

```sql
CREATE TABLE note_types (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  icon TEXT,
  color TEXT,
  schema_json TEXT NOT NULL,
  header_template_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
```

### Type definitions

Recommended field model:

```ts
type NoteFieldKind =
  | 'text'
  | 'long_text'
  | 'number'
  | 'date'
  | 'boolean'
  | 'select'
  | 'image'
  | 'url'

interface NoteTypeField {
  id: string
  label: string
  kind: NoteFieldKind
  required: boolean
  options?: string[]
  placeholder?: string
}

interface NoteTypeDefinition {
  id: string
  name: string
  slug: string
  icon?: string
  color?: string
  fields: NoteTypeField[]
}

interface HeaderTemplateDefinition {
  id: string
  kind: 'default' | 'centered_portrait' | 'compact_meta'
  config: Record<string, unknown>
}
```

### Renderer model

Each note becomes:

```ts
interface TypedEntry extends Entry {
  type_id: string | null
  header_layout: string | null
  header_props_json: string
  schema_version: number
}
```

### UI

Add a dedicated **Note Types** page/section where the user can:

- create a type
- define fields
- define a visual header layout
- choose icon/color/basic presentation

Editor becomes:

- `TypedHeaderRenderer`
- `EditorBody`

### Validation

In phase 1, Zod should be the TypeScript runtime validator.

Recommended approach:

1. store the type definition as JSON in SQLite
2. compile that definition into a Zod schema in renderer/main TS
3. validate `header_props_json` before save
4. keep compatibility defaults for legacy notes with no type metadata

### Important rule

The body remains the source of truth for the note document.
Header props do not replace the document editor.

## Phase 2: eden-heart boundary

### Goal

Move domain logic out of Node/Electron main into a Rust core binary, while keeping Electron + React as the shell/UI.

### Recommended shape

Use a **Rust sidecar/core** pattern.

Do not use a `.node` native addon as the primary architecture.
Do not rewrite to Tauri.

Use:

- Electron main process as lifecycle manager
- preload as safe bridge
- Rust sidecar binary as domain core

### Process boundary

Current flow:

`React -> preload -> ipcRenderer.invoke -> ipcMain.handle -> store.ts/search.ts`

Future flow:

`React -> preload -> ipcRenderer.invoke -> ipcMain.handle -> eden-heart client -> Rust core`

This means the initial contract should be extracted behind a service boundary in TS first.

### What moves into eden-heart

Eventually move:

- typed note validation
- note type CRUD
- entry CRUD
- folder tree logic
- search indexing/querying
- sync engine
- encryption engine
- conflict resolution

Keep in Electron shell:

- window controls
- file/folder picker dialogs
- UI state
- immediate editor rendering

## Phase 3: Search migration

### Current state

Search today is split:

- SQLite title search in `searchTitles()`
- ripgrep search over markdown files in `electron/search.ts`

That does not scale well for typed header properties.

### Recommended migration path

#### Step 1

Keep current search while typed notes land.

Add optional SQL search over promoted header fields only.

#### Step 2

When typed notes are stable, move search into `eden-heart` and replace ripgrep with a Rust-backed indexed engine.

Tantivy is a good fit for:

- title
- body text
- type id
- selected structured fields
- future filters/sorts

### Tantivy recommendation

Use Tantivy **inside `eden-heart`**, not as an isolated stopgap inside Node.

That gives one search system instead of:

- SQL titles
- ripgrep body
- ad hoc property filters

### Indexing model

Recommended indexed fields:

- `entry_id`
- `title`
- `body_text`
- `type_id`
- `folder_id`
- `updated_at`
- selected flattened header props

Not every field should be indexed equally.

## Zod + Rust validation model

Zod should validate on the TypeScript side.
Rust should validate the same logical schema independently.

Recommended rule:

- UI writes JSON type definitions
- TS compiles them to Zod schemas for local validation
- Rust compiles or interprets the same schema definition for domain validation

Do not make Zod the storage format.
Make JSON schema-like definitions the storage format.

## Safe implementation order

### First

1. add `note_types` table
2. extend `entries` with type/header metadata
3. create a minimal type registry in app
4. implement one header renderer path
5. include header metadata in autosave snapshot and save contract

### Then

6. allow user-defined fields and simple visual templates
7. add sidebar preview variants
8. add promoted-field sorting/filtering

### Then

9. extract domain service boundary in TS
10. introduce `eden-heart` as Rust sidecar
11. move search to Tantivy
12. move sync/encryption after search and contracts are stable

## What to avoid

Avoid these in phase 1:

- full relation/object graph
- arbitrary object-to-object schema engine
- table/gallery/database views
- full search rewrite before typed schema exists
- Rust rewrite before service boundaries are stable

## First implementation slice recommended now

The first concrete slice should be:

- add note types
- add typed visual headers
- keep body editor unchanged
- keep current search mostly unchanged
- keep all logic in TS/Electron for now

That is the highest-value, lowest-risk way to move Eden toward your actual product idea.
