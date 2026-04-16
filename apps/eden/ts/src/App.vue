<template>
  <!-- Loading state -->
  <div v-if="eden.isInitializing" class="app-container loading">
    <Titlebar />
    Загрузка...
  </div>

  <!-- Vault setup -->
  <div v-else-if="!eden.vaultPath" class="app-container setup-container">
    <Titlebar />
    <div class="setup-box">
      <h1>Добро пожаловать в Eden</h1>
      <p>Пожалуйста, выберите папку для хранения ваших заметок.</p>
      <button class="setup-btn" type="button" @click="eden.selectFolder()">Выбрать папку</button>
    </div>
  </div>

  <!-- Main app -->
  <div v-else :class="['app-container', { 'focus-mode-active': layout.isZenMode }]">
    <Titlebar v-if="!layout.isZenMode" />

    <SearchOverlay
      v-if="!layout.isZenMode"
      :is-open="layout.isSearchOpen"
      :query="pendingQuery"
      :results="layout.searchResults"
      :entry-titles="entryTitlesById"
      @query-change="pendingQuery = $event"
      @close="onSearchClose"
      @result-select="onResultSelect"
    />

    <div v-if="!layout.isZenMode && eden.activeScreen !== 'settings'" class="sidebar-layout">
      <EdenSidebar
        class="widget-sidebar-wrapper"
        :hidden="layout.widgetSidebarHidden"
        :initial-config="{ width: layout.widgetSidebarWidth, hidden: layout.widgetSidebarHidden }"
        :is-search-open="layout.isSearchOpen"
        :search-query="layout.searchQuery"
        :recent-entries="recentSidebarEntries"
        :current-entry="eden.currentEntry"
        @config-change="layout.onWidgetConfigChange"
        @update:hidden="layout.widgetSidebarHidden = $event"
        @toggle-search="layout.isSearchOpen = !layout.isSearchOpen"
        @create-entry="eden.createNewEntry()"
        @open-entry="(id) => eden.navigateTo(id)"
        @open-settings="eden.activeScreen = 'settings'"
      />
    </div>

    <main class="app-main">
      <button
        v-if="!layout.isZenMode && layout.widgetSidebarHidden"
        class="sidebar-head-icon withBackground sidebar-expand-btn"
        data-testid="sidebar-toggle-external"
        type="button"
        title="Открыть виджеты"
        @click="layout.toggleWidgetSidebar()"
      >
        <svg
          class="sidebar-expand-icon"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <rect width="18" height="18" x="3" y="3" rx="2" />
          <path d="M9 3v18" />
          <path d="m16 15-3-3 3-3" />
        </svg>
      </button>

      <SettingsPage
        v-if="eden.activeScreen === 'settings'"
        :settings="eden.codeToolsSettings"
        :vault-path="eden.vaultPath"
        :note-types="eden.noteTypes"
        :active-space="eden.activeSpace"
        :on-note-type-save="eden.saveNoteType"
        :on-note-type-delete="eden.deleteNoteType"
        @back="eden.activeScreen = 'notes'"
        @select-vault="eden.selectFolder()"
        @select-space="onSelectSpace"
        @settings-change="eden.updateCodeToolsSettings"
        @refresh-data="eden.refreshData()"
      />
      <Editor
        v-else-if="eden.currentEntry"
        :key="eden.currentEntry.id"
        :entry="eden.currentEntry"
        :all-entries="eden.entries"
        :note-types="eden.noteTypes"
        :code-tools-settings="eden.codeToolsSettings"
        :zen-mode="layout.isZenMode"
        :on-save="eden.handleSave"
        :on-navigate="eden.navigateTo"
        @exit-zen="layout.disableZenMode()"
      />
      <SpacesView
        v-else
        :active-space="eden.activeSpace"
        :entries="eden.entries"
        :note-types="eden.noteTypes"
        :sort-mode="eden.sortMode"
        @sort-mode-change="eden.sortMode = $event"
        @create-entry="eden.createNewEntry()"
        @open-entry="(id) => eden.navigateTo(id)"
      />
    </main>
    <CustomCaret />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, watch } from "vue";
import { CustomCaret } from "@kepler/visuals";
import { useEdenStore } from "@/store/eden";
import { useLayoutStore } from "@/store/layout";
import { useKeyboard } from "@/composables/useKeyboard";
import { usePlatform } from "@/composables/usePlatform";
import { useSearch } from "@/composables/useSearch";
import { useTitlebarSafeArea } from "@/composables/useTitlebarSafeArea";
import type { SpaceId } from "@/components/sidebar/types";
import Titlebar from "./Titlebar.vue";
import SearchOverlay from "@/components/SearchOverlay.vue";
import EdenSidebar from "@/components/sidebar/EdenSidebar.vue";
import Editor from "./Editor.vue";
import SpacesView from "@/components/spaces/SpacesView.vue";
import SettingsPage from "@/components/settings/SettingsPage.vue";
import "@/App.css";

const eden = useEdenStore();
const layout = useLayoutStore();

usePlatform();
useKeyboard();
useTitlebarSafeArea();
const { pendingQuery } = useSearch();

function pickRecentEntries(entries: Entry[], limit: number) {
  const topEntries: Entry[] = [];

  for (const entry of entries) {
    let insertAt = topEntries.findIndex(
      (candidate) => entry.updated_at > candidate.updated_at,
    );

    if (insertAt === -1) {
      if (topEntries.length >= limit) continue;
      insertAt = topEntries.length;
    }

    topEntries.splice(insertAt, 0, entry);

    if (topEntries.length > limit) {
      topEntries.length = limit;
    }
  }

  return topEntries;
}

const recentSidebarEntries = computed(() =>
  pickRecentEntries(eden.entries, 10),
);

const entryTitlesById = computed<Record<string, string>>(() =>
  Object.fromEntries(
    eden.entries.map((entry) => [entry.id, entry.title || "Без названия"]),
  ),
);

onMounted(() => {
  void eden.initApp();
});

// Auto-open my-space entry when needed
watch(
  [
    () => eden.isInitializing,
    () => eden.vaultPath,
    () => eden.activeScreen,
    () => eden.activeSpace,
    () => eden.currentEntry,
  ],
  () => {
    if (eden.isInitializing || !eden.vaultPath || eden.activeScreen !== "notes") return;
    if (eden.activeSpace === "my-space" && !eden.currentEntry) {
      void eden.openMySpace();
    }
  },
);

watch(
  [
    () => layout.isZenMode,
    () => eden.activeScreen,
    () => eden.currentEntry,
  ],
  ([isZenMode, activeScreen, currentEntry]) => {
    if (!isZenMode) return;

    layout.closeSearch();

    if (activeScreen !== "notes" || !currentEntry) {
      layout.disableZenMode();
    }
  },
);

function onSearchClose() {
  layout.isSearchOpen = false;
  pendingQuery.value = "";
  layout.searchQuery = "";
  layout.searchResults = [];
}

async function onResultSelect(entryId: string) {
  if (!window.api) return;
  const found = await window.api.loadEntry(entryId);
  if (found) {
    eden.activeScreen = "notes";
    eden.currentEntry = found;
    pendingQuery.value = "";
    layout.searchQuery = "";
    layout.searchResults = [];
    layout.isSearchOpen = false;
  }
}

async function onSelectSpace(spaceId: SpaceId) {
  await eden.refreshData();

  if (spaceId === "my-space") {
    void eden.openMySpace();
    return;
  }
  eden.activeScreen = "notes";
  eden.activeSpace = spaceId;
  eden.currentEntry = null;
}
</script>
