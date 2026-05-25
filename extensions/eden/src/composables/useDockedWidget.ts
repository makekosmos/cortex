// useDockedWidget — состояние "приложение в dock-corner режиме" (CSS marker
// `.eden-docked` на `.app-container`). Main process broadcast'ит изменения
// через `kepler.window.onDockedChange`. Auto-cleanup при unmount.

import { onMounted, onUnmounted, shallowRef } from "vue";

interface KeplerWindowApiExt {
  isDocked?: () => Promise<boolean>;
  onDockedChange?: (handler: (value: boolean) => void) => () => void;
}

export function useDockedWidget() {
  const isDocked = shallowRef(false);
  let offDockedChange: (() => void) | null = null;

  onMounted(async () => {
    const winApi = (
      window as unknown as { kepler?: { window?: KeplerWindowApiExt } }
    ).kepler?.window;
    if (!winApi) return;
    if (winApi.isDocked) {
      try {
        isDocked.value = await winApi.isDocked();
      } catch {
        // ignore
      }
    }
    if (winApi.onDockedChange) {
      offDockedChange = winApi.onDockedChange((value) => {
        isDocked.value = value;
      });
    }
  });

  onUnmounted(() => {
    try {
      offDockedChange?.();
    } catch {
      // ignore
    }
    offDockedChange = null;
  });

  return { isDocked };
}
