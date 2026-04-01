import { defineStore } from "pinia";
import { ref } from "vue";

const MIN_WIDGET_SIDEBAR_WIDTH = 220;
const MAX_WIDGET_SIDEBAR_WIDTH = 520;
const MIN_VAULT_SIDEBAR_WIDTH = 180;
const MAX_VAULT_SIDEBAR_WIDTH = 360;
const COLLAPSE_THRESHOLD = 60;

export const useLayoutStore = defineStore("layout", () => {
  const widgetSidebarWidth = ref(320);
  const widgetSidebarCollapsed = ref(false);
  const vaultSidebarWidth = ref(232);
  const vaultSidebarCollapsed = ref(false);
  const isSearchOpen = ref(false);
  const searchQuery = ref("");
  const searchResults = ref<SearchResult[]>([]);
  const sidebarAnimating = ref(false);
  const activeResizePanel = ref<"vault" | "widget" | null>(null);

  let animTimer: number | null = null;
  let isAnimating = false;
  let rafId: number | null = null;

  function startSidebarAnimation() {
    if (animTimer) window.clearTimeout(animTimer);
    isAnimating = true;
    sidebarAnimating.value = true;
    animTimer = window.setTimeout(() => {
      isAnimating = false;
      sidebarAnimating.value = false;
      animTimer = null;
    }, 220);
  }

  async function toggleWidgetSidebar() {
    startSidebarAnimation();
    widgetSidebarCollapsed.value = !widgetSidebarCollapsed.value;
    if (window.api) {
      await window.api.updateSidebarConfig({ widget: { collapsed: widgetSidebarCollapsed.value } });
    }
  }

  async function toggleVaultSidebar() {
    startSidebarAnimation();
    vaultSidebarCollapsed.value = !vaultSidebarCollapsed.value;
    if (window.api) {
      await window.api.updateSidebarConfig({ vault: { collapsed: vaultSidebarCollapsed.value } });
    }
  }

  function openSearch() {
    isSearchOpen.value = true;
  }

  function closeSearch() {
    isSearchOpen.value = false;
    searchQuery.value = "";
    searchResults.value = [];
  }

  function onResizeStart(panel: "vault" | "widget") {
    activeResizePanel.value = panel;
  }

  function onResizeMove(event: MouseEvent) {
    if (!activeResizePanel.value || isAnimating) return;
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      if (isAnimating || !activeResizePanel.value) return;

      if (activeResizePanel.value === "vault") {
        const newWidth = event.clientX;
        if (newWidth <= COLLAPSE_THRESHOLD) {
          if (!vaultSidebarCollapsed.value) {
            startSidebarAnimation();
            vaultSidebarCollapsed.value = true;
          }
          return;
        }
        if (vaultSidebarCollapsed.value) {
          startSidebarAnimation();
          vaultSidebarCollapsed.value = false;
          vaultSidebarWidth.value = MIN_VAULT_SIDEBAR_WIDTH;
          return;
        }
        vaultSidebarWidth.value = Math.max(
          MIN_VAULT_SIDEBAR_WIDTH,
          Math.min(MAX_VAULT_SIDEBAR_WIDTH, newWidth),
        );
        return;
      }

      const vaultVisible = vaultSidebarCollapsed.value ? 0 : vaultSidebarWidth.value;
      const newWidth = event.clientX - vaultVisible;
      if (newWidth <= COLLAPSE_THRESHOLD) {
        if (!widgetSidebarCollapsed.value) {
          startSidebarAnimation();
          widgetSidebarCollapsed.value = true;
        }
        return;
      }
      if (widgetSidebarCollapsed.value) {
        startSidebarAnimation();
        widgetSidebarCollapsed.value = false;
        widgetSidebarWidth.value = MIN_WIDGET_SIDEBAR_WIDTH;
        return;
      }
      widgetSidebarWidth.value = Math.max(
        MIN_WIDGET_SIDEBAR_WIDTH,
        Math.min(MAX_WIDGET_SIDEBAR_WIDTH, newWidth),
      );
    });
  }

  async function onResizeEnd() {
    if (rafId) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    activeResizePanel.value = null;
    if (window.api) {
      await window.api.updateSidebarConfig({
        widget: { width: widgetSidebarWidth.value, collapsed: widgetSidebarCollapsed.value },
        vault: { width: vaultSidebarWidth.value, collapsed: vaultSidebarCollapsed.value },
      });
    }
  }

  return {
    widgetSidebarWidth,
    widgetSidebarCollapsed,
    vaultSidebarWidth,
    vaultSidebarCollapsed,
    isSearchOpen,
    searchQuery,
    searchResults,
    sidebarAnimating,
    activeResizePanel,
    toggleWidgetSidebar,
    toggleVaultSidebar,
    openSearch,
    closeSearch,
    onResizeStart,
    onResizeMove,
    onResizeEnd,
  };
});
