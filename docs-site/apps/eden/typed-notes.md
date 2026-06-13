# Eden typed notes quick reference

Scope: `components/typed-notes/`, `lib/typedNotes.ts`, system typed objects in `lib/systemTypes.ts`.

- Built-in types include `note_obj`, `person_obj`, image/game/journal system types.
- `TypedHeader.vue` renders object header fields from `header_props_json`, `header_layout`, note type schema/ui schema.
- Person pages derive display name from `first_name`, `last_name`, `patronymic`; fallback is entry title/type name.
- Person/image visual fields may reference image objects; resolve through `lib/objectImages.ts` when available.
- Type changes must update `type_id`, `header_layout`, and `header_props_json` together; avoid editor-specific divergent metadata paths.
- Direct ARK calls from components are forbidden; use Eden API/shim/store paths.

Related: `docs-site/apps/eden/editor.md`, `docs-site/apps/eden/data.md`, `docs-site/agents/forbidden/eden.md`.
