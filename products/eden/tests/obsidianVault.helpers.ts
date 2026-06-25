import type { NoteType } from "../src/lib/typedNotes.js";

export function makeNoteType(overrides: Partial<NoteType> = {}): NoteType {
  return {
    id: "custom-note",
    name: "Custom Note",
    slug: "custom-note",
    icon: null,
    color: null,
    schema_json: JSON.stringify({ fields: [] }),
    header_template_json: JSON.stringify({
      kind: "default",
      primaryFieldIds: [],
      secondaryFieldIds: [],
      imageFieldId: null,
    }),
    ui_schema_json: JSON.stringify({}),
    created_at: 0,
    updated_at: 0,
    ...overrides,
  };
}
