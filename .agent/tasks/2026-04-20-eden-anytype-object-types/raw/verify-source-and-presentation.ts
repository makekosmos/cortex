import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  SYSTEM_TYPE_GAME,
  SYSTEM_TYPE_NOTE,
} from "../../../../apps/eden/ts/src/lib/systemTypes.ts";
import {
  createDefaultNoteTypeUiSchema,
  getNoteTypePresentation,
  parseNoteTypeUiSchema,
} from "../../../../apps/eden/ts/src/lib/typedNotes.ts";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

const repoRoot = path.resolve(import.meta.dir, "..", "..", "..", "..");

const typedHeaderSource = readFileSync(
  path.join(repoRoot, "apps/eden/ts/src/components/typed-notes/TypedHeader.vue"),
  "utf8",
);
const objectTypesSettingsSource = readFileSync(
  path.join(repoRoot, "apps/eden/ts/src/components/settings/ObjectTypesSettings.vue"),
  "utf8",
);
const editorSource = readFileSync(path.join(repoRoot, "apps/eden/ts/src/Editor.vue"), "utf8");
const storeSource = readFileSync(path.join(repoRoot, "apps/eden/ts/main/store.ts"), "utf8");

const notePresentation = getNoteTypePresentation(SYSTEM_TYPE_NOTE);
const gamePresentation = getNoteTypePresentation(SYSTEM_TYPE_GAME);
const defaultUiSchema = createDefaultNoteTypeUiSchema();
const parsedUiSchema = parseNoteTypeUiSchema(
  JSON.stringify({
    featured_fields: ["user_rating"],
    visible_fields: ["description", "user_rating"],
    hidden_fields: ["created_at", "updated_at", "deleted_at"],
    read_only_fields: ["total_playtime_seconds"],
    field_order: ["description", "user_rating"],
    header_layout: "column",
    default_layout: "page",
    default_template_id: null,
  }),
);

assert(notePresentation.headerLayout === "inline", "note_obj should default to inline header");
assert(
  notePresentation.descriptionField?.id === "description",
  "note_obj should expose description",
);
assert(
  notePresentation.secondaryFields.some((field) => field.id === "related_notes"),
  "note_obj should keep related_notes in secondary fields",
);

assert(gamePresentation.headerLayout === "column", "game_obj should default to column header");
assert(
  gamePresentation.descriptionField?.id === "description",
  "game_obj should expose description",
);
assert(
  gamePresentation.featuredFields.some((field) => field.id === "play_status"),
  "game_obj should expose play_status as featured",
);
assert(
  gamePresentation.featuredFields.some((field) => field.id === "total_playtime_seconds"),
  "game_obj should expose usage fields as featured",
);
assert(
  gamePresentation.secondaryFields.some((field) => field.id === "related_notes"),
  "game_obj should keep related_notes available",
);
assert(
  gamePresentation.imageFieldId === "cover_image",
  "game_obj should use cover_image as hero image",
);

assert(defaultUiSchema.header_layout === "inline", "default ui schema should use inline header");
assert(
  parsedUiSchema.header_layout === "column",
  "ui schema parser should preserve explicit column layout",
);
assert(
  parsedUiSchema.read_only_fields?.includes("total_playtime_seconds"),
  "ui schema parser should preserve read-only fields",
);

assert(
  typedHeaderSource.includes("typed-object-header__type-badge") &&
    typedHeaderSource.includes("typed-object-header__description") &&
    typedHeaderSource.includes("typed-object-header__properties--featured") &&
    typedHeaderSource.includes("typed-object-header__section-label"),
  "TypedHeader.vue should render type badge, description, featured properties, and secondary section",
);

assert(
  objectTypesSettingsSource.includes("Контракт защищён кодом") &&
    objectTypesSettingsSource.includes("Макет header") &&
    objectTypesSettingsSource.includes("В шапке") &&
    objectTypesSettingsSource.includes("В свойствах") &&
    objectTypesSettingsSource.includes("Скрыто") &&
    objectTypesSettingsSource.includes("Предпросмотр"),
  "ObjectTypesSettings.vue should expose lock chip, header layout, visibility modes, and preview",
);

const noteTypeIndex = editorSource.indexOf('<div ref="noteTypeMenuRef" class="note-type-inline">');
const titleIndex = editorSource.indexOf(
  '<input v-model="title" class="title-input" placeholder="Заголовок" />',
);
const typedHeaderIndex = editorSource.indexOf("<TypedHeader");
assert(
  noteTypeIndex !== -1 && titleIndex !== -1 && typedHeaderIndex !== -1,
  "Editor.vue should render note type, title, and typed header",
);
assert(noteTypeIndex < titleIndex, "Editor.vue should render note type before title");
assert(titleIndex < typedHeaderIndex, "Editor.vue should render title before typed header");

assert(
  storeSource.includes("related_notes: links") &&
    storeSource.includes('operation: "list_object_links"') &&
    storeSource.includes('operation: "upsert_object_link"') &&
    storeSource.includes('operation: "delete_object_link"'),
  "store.ts should hydrate and persist related_notes through object_links",
);

assert(
  storeSource.includes("uiSchemaJson: arkObjectType.uiSchemaJson") &&
    storeSource.includes("parseNoteTypeUiSchema(noteType.ui_schema_json)") &&
    storeSource.includes('operation: "upsert_object_type"'),
  "store.ts should persist note type presentation into Ark object types",
);

const report = {
  notePresentation,
  gamePresentation,
  defaultUiSchema,
  parsedUiSchema,
  sourceChecks: {
    typedHeader: true,
    objectTypesSettings: true,
    editorOrder: true,
    storeObjectLinks: true,
    storePresentationPersistence: true,
  },
};

const outPath = path.join(import.meta.dir, "source-presentation-verification.json");
writeFileSync(outPath, JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
