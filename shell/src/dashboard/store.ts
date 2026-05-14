// Compositional store для Dashboard. Минимальный — Vue ref'ы + загрузка
// через `window.kepler` IPC API (см. preload.ts).
//
// Намерено НЕ использует Pinia: dashboard живёт изолированно в своём окне,
// state per-window, hot-reload и debugging проще через нативные ref'ы.

import { ref } from "vue";
import type { SpaceMeta } from "./types";

export const spaces = ref<SpaceMeta[]>([]);
export const spacesLoading = ref<boolean>(false);
export const spacesError = ref<string | null>(null);

export const selectedSpaceId = ref<string | null>(null);

export async function loadSpaces(): Promise<void> {
  spacesLoading.value = true;
  spacesError.value = null;
  try {
    const list = await window.kepler.spaces.list();
    spaces.value = list;
  } catch (e) {
    spacesError.value = e instanceof Error ? e.message : String(e);
    spaces.value = [];
  } finally {
    spacesLoading.value = false;
  }
}
