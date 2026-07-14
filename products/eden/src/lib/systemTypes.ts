import type { NoteType } from "./typedNotes";
import {
  SYSTEM_TYPES,
  SYSTEM_TYPE_BOOK_ID,
  SYSTEM_TYPE_COLLECTION_ID,
  SYSTEM_TYPE_EXERCISE_ID,
  SYSTEM_TYPE_GAME_ID,
  SYSTEM_TYPE_IMAGE_ID,
  SYSTEM_TYPE_JOURNAL_ID,
  SYSTEM_TYPE_NOTE_ID,
  SYSTEM_TYPE_PERSON_ID,
  SYSTEM_TYPE_WORKOUT_ID,
  gameHeaderTemplateJson,
  gameSchemaJson,
  gameUiSchemaJson,
  shouldUpgradeLegacyGamePresentation,
  shouldUpgradeLegacyGameSchema,
} from "./systemTypeDefinitions";

export {
  SYSTEM_TYPES,
  SYSTEM_TYPE_BOOK,
  SYSTEM_TYPE_BOOK_ID,
  SYSTEM_TYPE_COLLECTION,
  SYSTEM_TYPE_COLLECTION_ID,
  SYSTEM_TYPE_IMAGE,
  SYSTEM_TYPE_IMAGE_ID,
  SYSTEM_TYPE_JOURNAL,
  SYSTEM_TYPE_JOURNAL_ID,
  SYSTEM_TYPE_NOTE,
  SYSTEM_TYPE_NOTE_ID,
  SYSTEM_TYPE_PERSON,
  SYSTEM_TYPE_PERSON_ID,
  SYSTEM_TYPE_GAME_ID,
} from "./systemTypeDefinitions";

const HIDDEN_EDEN_COLLECTION_TYPE_IDS = new Set([
  SYSTEM_TYPE_COLLECTION_ID,
  SYSTEM_TYPE_JOURNAL_ID,
  "blocklist_obj",
  "tag_obj",
  "task_obj",
  "time_entry_obj",
]);

export function normalizeSystemNoteType(noteType: NoteType): NoteType {
  const systemType = SYSTEM_TYPES.find((candidate) => candidate.id === noteType.id);
  if (systemType && noteType.id !== SYSTEM_TYPE_GAME_ID) {
    return {
      ...systemType,
      created_at: noteType.created_at || systemType.created_at,
      updated_at: noteType.updated_at || systemType.updated_at,
    };
  }

  if (noteType.id !== SYSTEM_TYPE_GAME_ID) {
    return noteType;
  }

  if (
    shouldUpgradeLegacyGameSchema(noteType) ||
    !noteType.ui_schema_json?.trim() ||
    shouldUpgradeLegacyGamePresentation(noteType)
  ) {
    return {
      ...noteType,
      schema_json: gameSchemaJson,
      header_template_json: gameHeaderTemplateJson,
      ui_schema_json: gameUiSchemaJson,
    };
  }

  return noteType;
}

export function isSystemType(noteTypeId: string): boolean {
  return (
    noteTypeId === SYSTEM_TYPE_NOTE_ID ||
    noteTypeId === SYSTEM_TYPE_BOOK_ID ||
    noteTypeId === SYSTEM_TYPE_COLLECTION_ID ||
    noteTypeId === SYSTEM_TYPE_JOURNAL_ID ||
    noteTypeId === SYSTEM_TYPE_IMAGE_ID ||
    noteTypeId === SYSTEM_TYPE_PERSON_ID ||
    noteTypeId === SYSTEM_TYPE_GAME_ID ||
    noteTypeId === SYSTEM_TYPE_WORKOUT_ID ||
    noteTypeId === SYSTEM_TYPE_EXERCISE_ID
  );
}

export function shouldShowAsEdenCollection(noteTypeId: string): boolean {
  return !HIDDEN_EDEN_COLLECTION_TYPE_IDS.has(noteTypeId);
}
