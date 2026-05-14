// Compositional store для Dashboard. Минимальный — Vue ref'ы + загрузка
// через `window.kepler` IPC API (см. preload.ts).
//
// Намерено НЕ использует Pinia: dashboard живёт изолированно в своём окне,
// state per-window, hot-reload и debugging проще через нативные ref'ы.
//
// C2: scaffold only — welcome view рендерит UI, но space list пустой (preload
// API kepler:spaces:list появится в C3). C4 добавит loadObjectTypes /
// loadObjects для space view.

import { ref } from "vue";
import type { SpaceMeta } from "./types";

export const spaces = ref<SpaceMeta[]>([]);
export const spacesLoading = ref<boolean>(false);
export const spacesError = ref<string | null>(null);

export const selectedSpaceId = ref<string | null>(null);

export async function loadSpaces(): Promise<void> {
  // C2: preload API ещё не подключён — рендерим пустой список.
  spaces.value = [];
}
