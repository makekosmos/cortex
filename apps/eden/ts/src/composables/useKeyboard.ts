import { onMounted, onUnmounted } from "vue";

import { useLayoutStore } from "@/store/layout";

const ZOOM_STEP = 0.1;

const ZOOM_MIN = 0.5;

const ZOOM_MAX = 2.0;

export function useKeyboard() {
  const layout = useLayoutStore();

  async function restoreZoom() {
    const saved = localStorage.getItem("eden-zoom");

    if (saved && window.api?.zoomSet) {
      await window.api.zoomSet(parseFloat(saved));
    }
  }

  async function handleKeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;

    // Zoom

    if (mod && window.api?.zoomGet) {
      let next: number | null = null;

      if (e.key === "=" || e.key === "+") {
        const cur = await window.api.zoomGet();

        next = Math.min(ZOOM_MAX, Math.round((cur + ZOOM_STEP) * 100) / 100);
      } else if (e.key === "-") {
        const cur = await window.api.zoomGet();

        next = Math.max(ZOOM_MIN, Math.round((cur - ZOOM_STEP) * 100) / 100);
      } else if (e.key === "0") {
        next = 1;
      }

      if (next !== null) {
        e.preventDefault();

        const applied = await window.api.zoomSet(next);

        localStorage.setItem("eden-zoom", String(applied));

        return;
      }
    }

    // Cmd+K — toggle search

    if (mod && e.key === "k") {
      e.preventDefault();

      if (layout.isSearchOpen) {
        layout.closeSearch();
      } else {
        layout.openSearch();
      }

      return;
    }

    // Escape — close search

    if (e.key === "Escape" && layout.isSearchOpen) {
      layout.closeSearch();
    }
  }

  onMounted(() => {
    void restoreZoom();

    window.addEventListener("keydown", handleKeydown);
  });

  onUnmounted(() => {
    window.removeEventListener("keydown", handleKeydown);
  });
}
