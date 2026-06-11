import { onMounted, onUnmounted } from "vue";

import { useEdenStore } from "@/store/eden";

import { useLayoutStore } from "@/store/layout";

const ZOOM_STEP = 0.1;

const ZOOM_MIN = 0.5;

const ZOOM_MAX = 2.0;

// Chord-shortcut config. Ctrl+K — Z (VS Code style) → toggle zen.
// Ctrl+K сам по себе открывает поиск (legacy behavior); если в течение
// CHORD_WINDOW_MS пользователь нажмёт Z — отменяем search и переключаем zen.
const CHORD_WINDOW_MS = 700;

// Double-Esc для выхода из zen mode. Если два Escape подряд в пределах
// этого окна — отключаем zen. Одиночный Escape ничего не делает (чтобы
// пользователь случайным касанием не вылетал из режима фокуса).
const DOUBLE_ESC_WINDOW_MS = 600;

export function useKeyboard() {
  const eden = useEdenStore();

  const layout = useLayoutStore();

  let chordExpiresAt = 0;
  let lastEscapeAt = 0;

  async function restoreZoom() {
    const saved = localStorage.getItem("eden-zoom");

    if (saved && window.api?.zoomSet) {
      await window.api.zoomSet(parseFloat(saved));
    }
  }

  async function handleKeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;

    // Используем e.code (физическая клавиша) для буквенных хоткеев, чтобы
    // они отрабатывали независимо от раскладки (RU/EN). e.key зависит от
    // активной layout'ы — на русской "K" даёт "Л", "Z" даёт "Я".
    // Не-буквенные хоткеи (Equal/Minus/Digit0/Escape) e.code и так стабилен.

    // Zoom

    if (mod && window.api?.zoomGet) {
      let next: number | null = null;

      if (e.code === "Equal" || e.code === "NumpadAdd") {
        const cur = await window.api.zoomGet();

        next = Math.min(ZOOM_MAX, Math.round((cur + ZOOM_STEP) * 100) / 100);
      } else if (e.code === "Minus" || e.code === "NumpadSubtract") {
        const cur = await window.api.zoomGet();

        next = Math.max(ZOOM_MIN, Math.round((cur - ZOOM_STEP) * 100) / 100);
      } else if (e.code === "Digit0" || e.code === "Numpad0") {
        next = 1;
      }

      if (next !== null) {
        e.preventDefault();

        const applied = await window.api.zoomSet(next);

        localStorage.setItem("eden-zoom", String(applied));

        return;
      }
    }

    // Ctrl/Cmd+B — раскрыть / скрыть sidebar. Используем physical code,
    // чтобы shortcut не зависел от текущей раскладки.
    if (mod && !e.altKey && e.code === "KeyB") {
      e.preventDefault();
      e.stopPropagation();
      await layout.toggleWidgetSidebar();
      console.debug("[eden] sidebar toggled by keyboard", {
        hidden: layout.widgetSidebarHidden,
      });
      return;
    }

    // Ctrl+K — открыть поиск + поднять chord-окно. Если за CHORD_WINDOW_MS
    // успели нажать Z — это chord Ctrl+K Z, переключаем zen.

    if (mod && e.code === "KeyK") {
      if (layout.isZenMode) {
        // В zen mode Ctrl+K = только инициирование chord'а (chord toggle'нет
        // zen обратно). Search не открываем — он недоступен в zen.
        chordExpiresAt = Date.now() + CHORD_WINDOW_MS;
        e.preventDefault();
        return;
      }

      e.preventDefault();

      if (layout.isSearchOpen) {
        layout.closeSearch();
      } else {
        layout.openSearch();
      }

      chordExpiresAt = Date.now() + CHORD_WINDOW_MS;
      return;
    }

    // Chord Ctrl+K → Z — переключить zen mode.

    if (!mod && e.code === "KeyZ" && chordExpiresAt > 0 && Date.now() <= chordExpiresAt) {
      e.preventDefault();
      chordExpiresAt = 0;

      // Откатываем побочный эффект Ctrl+K (search открылся как fallback).
      if (layout.isSearchOpen) {
        layout.closeSearch();
      }

      if (eden.activeScreen !== "notes" || !eden.currentEntry) {
        return;
      }

      layout.toggleZenMode();
      return;
    }

    // Legacy: Cmd/Ctrl+Alt+Z — оставлен как альтернатива chord'у на случай
    // привычки пользователя.

    if (mod && e.altKey && e.code === "KeyZ") {
      if (eden.activeScreen !== "notes" || !eden.currentEntry) {
        return;
      }

      e.preventDefault();
      layout.toggleZenMode();
      return;
    }

    // Escape:
    //   - search открыт → закрываем (одиночный Esc, как везде)
    //   - в zen → требуем двойной Esc в окне DOUBLE_ESC_WINDOW_MS

    if (e.code === "Escape") {
      if (layout.isSearchOpen) {
        layout.closeSearch();
        lastEscapeAt = 0;
        return;
      }

      if (layout.isZenMode) {
        const now = Date.now();
        if (now - lastEscapeAt <= DOUBLE_ESC_WINDOW_MS && lastEscapeAt > 0) {
          layout.disableZenMode();
          lastEscapeAt = 0;
        } else {
          lastEscapeAt = now;
        }
      }
    }
  }

  onMounted(() => {
    void restoreZoom();

    window.addEventListener("keydown", handleKeydown, { capture: true });
    document.addEventListener("keydown", handleKeydown, { capture: true });
  });

  onUnmounted(() => {
    window.removeEventListener("keydown", handleKeydown, { capture: true });
    document.removeEventListener("keydown", handleKeydown, { capture: true });
  });
}
