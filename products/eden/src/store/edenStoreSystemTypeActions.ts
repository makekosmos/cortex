import { SYSTEM_TYPES_BY_ID } from "./edenStoreHelpers";

export async function ensureSystemTypePersisted(noteTypeId: string): Promise<void> {
  const systemType = SYSTEM_TYPES_BY_ID.get(noteTypeId);
  if (!systemType || !window.api) return;

  const result = await window.api.saveNoteType(systemType);
  if (!result.ok) {
    console.warn("[eden] persist system type failed:", result);
  }
}
