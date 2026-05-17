import { defineStore } from "pinia";

import { ref } from "vue";

const MIN_WIDGET_SIDEBAR_WIDTH = 220;

const MAX_WIDGET_SIDEBAR_WIDTH = 520;

export const useLayoutStore = defineStore("layout", () => {
  const widgetSidebarWidth = ref(320);

  const widgetSidebarHidden = ref(false);

  const isSearchOpen = ref(false);

  const isZenMode = ref(false);

  const searchQuery = ref("");

  const searchResults = ref<SearchResult[]>([]);

  async function toggleWidgetSidebar() {
    widgetSidebarHidden.value = !widgetSidebarHidden.value;

    if (window.api) {
      await window.api.updateSidebarConfig({
        widget: {
          hidden: widgetSidebarHidden.value,
          width: widgetSidebarWidth.value,
        },
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

  function enableZenMode() {
    isZenMode.value = true;
    closeSearch();
  }

  function disableZenMode() {
    isZenMode.value = false;
  }

  function toggleZenMode() {
    if (isZenMode.value) {
      disableZenMode();
      return;
    }

    enableZenMode();
  }

  async function onWidgetConfigChange(config: {
    width: number;
    hidden: boolean;
  }) {
    widgetSidebarWidth.value = Math.max(
      MIN_WIDGET_SIDEBAR_WIDTH,

      Math.min(MAX_WIDGET_SIDEBAR_WIDTH, config.width),
    );

    widgetSidebarHidden.value = config.hidden;

    if (window.api) {
      await window.api.updateSidebarConfig({ widget: config });
    }
  }

  return {
    widgetSidebarWidth,

    widgetSidebarHidden,

    isSearchOpen,

    isZenMode,

    searchQuery,

    searchResults,

    toggleWidgetSidebar,

    openSearch,

    closeSearch,

    enableZenMode,

    disableZenMode,

    toggleZenMode,

    onWidgetConfigChange,
  };
});
