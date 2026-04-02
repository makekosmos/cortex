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
  <div v-else class="app-container">
    <Titlebar />

    <SearchOverlay
      :is-open="layout.isSearchOpen"
      :query="pendingQuery"
      :results="layout.searchResults"
      :entries="eden.entries"
      @query-change="pendingQuery = $event"
      @close="onSearchClose"
      @result-select="onResultSelect"
    />

    <div
      class="sidebar-layout"
      :style="eden.activeScreen === 'settings' ? { display: 'none' } : undefined"
    >
      <!-- Vault sidebar -->
      <ResizableSidebar
        v-model:collapsed="layout.vaultSidebarCollapsed"
        :default-width="232"
        :min-width="180"
        :max-width="360"
        :collapse-threshold="60"
        :initial-config="{ width: layout.vaultSidebarWidth, collapsed: layout.vaultSidebarCollapsed }"
        @config-change="layout.onVaultConfigChange"
      >
        <template #default="{ toggle }">
          <VaultSidebar
            :vault-path="eden.vaultPath"
            :recent-vault-paths="eden.recentVaultPaths"
            :collapsed="layout.vaultSidebarCollapsed"
            @toggle-collapsed="toggle"
            @select-vault="(path) => eden.selectVaultPath(path)"
            @open-vault-picker="eden.selectFolder()"
          />
        </template>
      </ResizableSidebar>

      <!-- Widget sidebar -->
      <ResizableSidebar
        v-model:collapsed="layout.widgetSidebarCollapsed"
        :default-width="320"
        :min-width="220"
        :max-width="520"
        :collapse-threshold="60"
        :offset-x="layout.vaultSidebarCollapsed ? 0 : layout.vaultSidebarWidth"
        :initial-config="{ width: layout.widgetSidebarWidth, collapsed: layout.widgetSidebarCollapsed }"
        @config-change="layout.onWidgetConfigChange"
      >
        <template #default="{ toggle }">
          <WidgetSidebar
            :is-search-open="layout.isSearchOpen"
            :is-vault-sidebar-collapsed="layout.vaultSidebarCollapsed"
            :active-space="eden.activeSpace"
            :entries="eden.entries"
            :note-types="eden.noteTypes"
            :current-entry="eden.currentEntry"
            :search-query="layout.searchQuery"
            @toggle-collapse="toggle"
            @toggle-vault-sidebar="layout.toggleVaultSidebar()"
            @select-space="onSelectSpace"
            @search-toggle="layout.isSearchOpen = !layout.isSearchOpen"
            @create-root-entry="eden.createNewEntry()"
            @open-entry="(id) => eden.navigateTo(id)"
            @open-settings="eden.activeScreen = 'settings'"
          />
        </template>
      </ResizableSidebar>
    </div>

    <main class="app-main">
      <button
        v-if="layout.widgetSidebarCollapsed"
        class="sidebar-head-icon withBackground sidebar-expand-btn"
        data-testid="sidebar-toggle-external"
        type="button"
        title="Открыть виджеты"
        @click="layout.toggleWidgetSidebar()"
      >
        <span aria-hidden="true" class="anytype-icon toggleWidget" />
      </button>

      <SettingsPage
        v-if="eden.activeScreen === 'settings'"
        :settings="eden.codeToolsSettings"
        :vault-path="eden.vaultPath"
        :note-types="eden.noteTypes"
        :on-note-type-save="eden.saveNoteType"
        :on-note-type-delete="eden.deleteNoteType"
        @back="eden.activeScreen = 'notes'"
        @select-vault="eden.selectFolder()"
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
        :on-save="eden.handleSave"
        :on-navigate="eden.navigateTo"
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
  </div>
</template>

<script setup lang="ts">
import { onMounted, watch } from "vue";
import { useEdenStore } from "@/store/eden";
import { useLayoutStore } from "@/store/layout";
import { useKeyboard } from "@/composables/useKeyboard";
import { usePlatform } from "@/composables/usePlatform";
import { useSearch } from "@/composables/useSearch";
import { useTitlebarSafeArea } from "@/composables/useTitlebarSafeArea";
import type { SpaceId } from "@/components/sidebar/types";
import ResizableSidebar from "@kepler/visuals/components/ResizableSidebar.vue";
import Titlebar from "./Titlebar.vue";
import SearchOverlay from "@/components/SearchOverlay.vue";
import VaultSidebar from "@/components/sidebar/VaultSidebar.vue";
import WidgetSidebar from "@/components/sidebar/WidgetSidebar.vue";
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

function onSelectSpace(spaceId: SpaceId) {
  if (spaceId === "my-space") {
    void eden.openMySpace();
    return;
  }
  eden.activeScreen = "notes";
  eden.activeSpace = spaceId;
  eden.currentEntry = null;
}
</script>
