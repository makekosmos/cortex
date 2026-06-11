import { defineStore } from "pinia";

import { ref } from "vue";

const EDEN_SETTINGS_SIDEBAR_WIDTH = 228;

// LocalStorage key для persistence zen mode'а. Юзер ожидает что reload
// окна (особенно дев-HMR) сохраняет состояние «я был в zen режиме».
const ZEN_MODE_STORAGE_KEY = "eden:layout:zenMode";

function readPersistedZenMode(): boolean {
  try {
    return window.localStorage.getItem(ZEN_MODE_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function writePersistedZenMode(value: boolean): void {
  try {
    if (value) {
      window.localStorage.setItem(ZEN_MODE_STORAGE_KEY, "1");
    } else {
      window.localStorage.removeItem(ZEN_MODE_STORAGE_KEY);
    }
  } catch {
    // localStorage может быть disabled (private mode) — silent ignore.
  }
}

export const useLayoutStore = defineStore("layout", () => {
  const widgetSidebarHidden = ref(false);

  const isSearchOpen = ref(false);

  const isZenMode = ref(readPersistedZenMode());

  const searchQuery = ref("");

  const searchResults = ref<SearchResult[]>([]);

  async function toggleWidgetSidebar() {
    widgetSidebarHidden.value = !widgetSidebarHidden.value;

    if (window.api) {
      await window.api.updateSidebarConfig({
        widget: {
          hidden: widgetSidebarHidden.value,
          width: EDEN_SETTINGS_SIDEBAR_WIDTH,
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
    writePersistedZenMode(true);
    closeSearch();
  }

  function disableZenMode() {
    isZenMode.value = false;
    writePersistedZenMode(false);
  }

  function toggleZenMode() {
    if (isZenMode.value) {
      disableZenMode();
      return;
    }

    enableZenMode();
  }

  async function onWidgetConfigChange(config: { width: number; hidden: boolean }) {
    widgetSidebarHidden.value = config.hidden;

    if (window.api) {
      await window.api.updateSidebarConfig({
        widget: { hidden: config.hidden, width: EDEN_SETTINGS_SIDEBAR_WIDTH },
      });
    }
  }

  return {
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
