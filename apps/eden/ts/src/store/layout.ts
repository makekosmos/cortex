import { defineStore } from "pinia";
import { ref } from "vue";

const MIN_WIDGET_SIDEBAR_WIDTH = 220;
const MAX_WIDGET_SIDEBAR_WIDTH = 520;
const MIN_VAULT_SIDEBAR_WIDTH = 180;
const MAX_VAULT_SIDEBAR_WIDTH = 360;

export const useLayoutStore = defineStore("layout", () => {
  const widgetSidebarWidth = ref(320);
  const widgetSidebarCollapsed = ref(false);
  const vaultSidebarWidth = ref(232);
  const vaultSidebarCollapsed = ref(false);
  const isSearchOpen = ref(false);
  const searchQuery = ref("");
  const searchResults = ref<SearchResult[]>([]);

  async function toggleWidgetSidebar() {
    widgetSidebarCollapsed.value = !widgetSidebarCollapsed.value;
    if (window.api) {
      await window.api.updateSidebarConfig({
        widget: { collapsed: widgetSidebarCollapsed.value, width: widgetSidebarWidth.value },
      });
    }
  }

  async function toggleVaultSidebar() {
    vaultSidebarCollapsed.value = !vaultSidebarCollapsed.value;
    if (window.api) {
      await window.api.updateSidebarConfig({
        vault: { collapsed: vaultSidebarCollapsed.value, width: vaultSidebarWidth.value },
      });
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

  async function onVaultConfigChange(config: { width: number; collapsed: boolean }) {
    vaultSidebarWidth.value = Math.max(
      MIN_VAULT_SIDEBAR_WIDTH,
      Math.min(MAX_VAULT_SIDEBAR_WIDTH, config.width),
    );
    vaultSidebarCollapsed.value = config.collapsed;
    if (window.api) {
      await window.api.updateSidebarConfig({ vault: config });
    }
  }

  async function onWidgetConfigChange(config: { width: number; collapsed: boolean }) {
    widgetSidebarWidth.value = Math.max(
      MIN_WIDGET_SIDEBAR_WIDTH,
      Math.min(MAX_WIDGET_SIDEBAR_WIDTH, config.width),
    );
    widgetSidebarCollapsed.value = config.collapsed;
    if (window.api) {
      await window.api.updateSidebarConfig({ widget: config });
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
    toggleWidgetSidebar,
    toggleVaultSidebar,
    openSearch,
    closeSearch,
    onVaultConfigChange,
    onWidgetConfigChange,
  };
});
