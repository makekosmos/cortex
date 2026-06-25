import type { Ref } from "vue";

import { v4 as uuidv4 } from "uuid";

import { isSystemType } from "@/lib/systemTypes";
import { normalizeSlug } from "@/lib/typedNotes";

interface EdenStoreNoteTypeActionState {
  noteTypes: Ref<NoteType[]>;
  refreshData(): Promise<void>;
}

export function createEdenStoreNoteTypeActions(state: EdenStoreNoteTypeActionState) {
  async function saveNoteType(
    draft: Omit<NoteType, "id" | "created_at" | "updated_at" | "slug"> & {
      id?: string;
      slug?: string;
    },
  ): Promise<SaveNoteTypeResult> {
    if (!window.api) return { ok: false, reason: "invalid_definition", message: "No API" };

    const now = Date.now();
    const noteType: NoteType = {
      id: draft.id ?? uuidv4(),
      name: draft.name,
      slug: normalizeSlug(draft.slug || draft.name),
      icon: draft.icon,
      color: draft.color,
      schema_json: draft.schema_json,
      header_template_json: draft.header_template_json,
      ui_schema_json: draft.ui_schema_json,
      created_at: draft.id
        ? (state.noteTypes.value.find((type) => type.id === draft.id)?.created_at ?? now)
        : now,
      updated_at: now,
    };

    const result = await window.api.saveNoteType(noteType);
    if (result.ok) await state.refreshData();
    return result;
  }

  async function deleteNoteType(noteTypeId: string) {
    if (!window.api || isSystemType(noteTypeId)) return;

    await window.api.deleteNoteType(noteTypeId);
    await state.refreshData();
  }

  return { saveNoteType, deleteNoteType };
}
