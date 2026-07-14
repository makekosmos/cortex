# Eden typed notes quick reference

Scope: `components/typed-notes/`, `lib/typedNotes.ts`, system typed objects in `lib/systemTypes.ts`.

- Built-in types include `note_obj`, `book_obj`, `person_obj`, image/game/journal system types.
- `book_obj` stores its title in `Entry.title`, optional `author` and `cover_image` in `header_props_json`, and arbitrary book notes in the Markdown body. `cover_image` is an `image` field and the type's `imageFieldId`; the collection is «Книги» with the existing `book` icon and accent fallback.
- Books use the same system-type persistence and ARK write path as other Eden objects; do not add a book-specific RPC or database schema.
- `TypedHeader.vue` renders object header fields from `header_props_json`, `header_layout`, note type schema/ui schema.
- Person pages derive display name from `first_name`, `last_name`, `patronymic`; fallback is entry title/type name.
- Person/image visual fields may reference image objects; resolve through `lib/objectImages.ts` when available.
- Type changes must update `type_id`, `header_layout`, and `header_props_json` together; avoid editor-specific divergent metadata paths.
- Direct ARK calls from components are forbidden; use Eden API/shim/store paths.

Related: `docs-site/apps/eden/editor.md`, `docs-site/apps/eden/data.md`, `docs-site/agents/forbidden/eden.md`.
