# typedNotes dead export cleanup

Scope: `products/eden/src/lib/typedNotes.ts`

Goal:

- Remove `export` from dead declarations that have no external repo usage.
- Keep declarations and runtime behavior intact.
- Leave unrelated edits untouched.

Verified target names:

- `noteFieldKinds`
- `legacyHeaderTemplateKinds`
- `objectHeaderLayoutKinds`
- `objectDefaultLayoutKinds`
- `NoteFieldKind`
- `LegacyHeaderTemplateKind`
- `ObjectDefaultLayoutKind`
- `NoteFieldDisplayMode`
- `NoteTypeField`
- `HeaderTemplateDefinition`
- `noteTypeDefinitionSchema`
- `createDefaultHeaderTemplate`
- `createDefaultNoteTypeDefinition`
- `createDefaultNoteTypeUiSchema`
- `pluralizeNoteTypeName`
- `getNoteTypeFieldDisplayMode`
- `createDefaultHeaderProps`
- `buildHeaderPropsSchema`
