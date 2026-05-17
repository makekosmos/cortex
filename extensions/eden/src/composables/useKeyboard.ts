import { onMounted, onUnmounted } from "vue";

import { useEdenStore } from "@/store/eden";

import { useLayoutStore } from "@/store/layout";

const ZOOM_STEP = 0.1;

const ZOOM_MIN = 0.5;

const ZOOM_MAX = 2.0;

export function useKeyboard() {
  const eden = useEdenStore();

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
      if (layout.isZenMode) {
        return;
      }

      e.preventDefault();

      if (layout.isSearchOpen) {
        layout.closeSearch();
      } else {
        layout.openSearch();
      }

      return;
    }

    // Cmd/Ctrl+Alt+Z — toggle zen mode

    if (mod && e.altKey && e.key.toLowerCase() === "z") {
      if (eden.activeScreen !== "notes" || !eden.currentEntry) {
        return;
      }

      e.preventDefault();
      layout.toggleZenMode();
      return;
    }

    // Escape — close search / exit zen

    if (e.key === "Escape") {
      if (layout.isSearchOpen) {
        layout.closeSearch();
      }

      if (layout.isZenMode) {
        layout.disableZenMode();
      }
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
