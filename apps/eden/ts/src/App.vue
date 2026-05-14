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

    <DesktopChrome
      v-if="!layout.isZenMode"
      class="app-shell"
      :platform="chromePlatform"
    >
      <template #titlebar-leading>
        <button
          type="button"
          class="sidebar-head-icon withBackground eden-titlebar-toggle"
          data-testid="sidebar-toggle"
          :title="layout.widgetSidebarHidden ? 'Показать боковую панель' : 'Скрыть боковую панель'"
          @click="layout.toggleWidgetSidebar()"
        >
          <svg
            width="14"
            height="14"
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
          </svg>
        </button>

        <TitlebarHistoryControls
          :back-disabled="!canGoBack"
          :forward-disabled="!canGoForward"
          back-title="Назад"
          forward-title="Вперёд"
          @back="navigateBack"
          @forward="navigateForward"
        />
      </template>

      <template
        v-if="showSidebarChrome"
        #sidebar
      >
        <div class="sidebar-layout">
          <EdenSidebar
            class="widget-sidebar-wrapper"
            :hidden="layout.widgetSidebarHidden"
            :initial-config="{ width: layout.widgetSidebarWidth, hidden: layout.widgetSidebarHidden }"
            :is-search-open="layout.isSearchOpen"
            :search-query="layout.searchQuery"
            :recent-entries="recentSidebarEntries"
            :all-entries="eden.entries"
            :note-types="eden.noteTypes"
            :current-entry="eden.currentEntry"
            :active-screen="eden.activeScreen"
            :active-settings-tab="settingsInitialTab"
            :selected-object-type-id="eden.activeNoteTypeId"
            @config-change="layout.onWidgetConfigChange"
            @update:hidden="layout.widgetSidebarHidden = $event"
            @toggle-search="layout.isSearchOpen = !layout.isSearchOpen"
            @create-entry="eden.createNewEntry()"
            @open-entry="(id) => eden.navigateTo(id)"
            @open-settings-tab="openSettingsTab"
            @open-object-types="openObjectTypes()"
            @open-object-type="eden.openTypeCollection($event)"
            @create-object-type="createObjectType()"
            @back="handleSidebarBack"
          />
        </div>
      </template>

      <DesktopContentSurface
        class="eden-content-surface"
        padding-top="0"
        padding-inline="0"
        padding-bottom="0"
        :show-left-border="showSidebarChrome && !layout.widgetSidebarHidden"
        :radius-top-left="showSidebarChrome && !layout.widgetSidebarHidden ? '16px' : '0px'"
      >
        <main class="app-main">
          <div
            v-if="eden.isHydratingVault && eden.activeScreen !== 'settings' && eden.activeScreen !== 'object-types' && !eden.currentEntry"
            class="app-main-loading"
          >
            Загрузка данных...
          </div>
          <ObjectTypesSettings
            v-else-if="eden.activeScreen === 'object-types'"
            :note-types="eden.noteTypes"
            :initial-selected-type-id="eden.activeNoteTypeId"
            :create-draft-token="objectTypeCreateToken"
            :on-note-type-save="eden.saveNoteType"
            :on-note-type-delete="eden.deleteNoteType"
            @selected-type-change="onSelectedTypeChange"
          />
          <TypeObjectsView
            v-else-if="eden.activeScreen === 'type-collection' && activeCollectionType"
            :note-type="activeCollectionType"
            :entries="eden.entries"
            @open-entry="(id) => eden.navigateTo(id)"
            @create-entry="eden.createNewEntry(activeCollectionType.id)"
            @edit-type="openTypeSettings(activeCollectionType.id)"
          />
          <SettingsPage
            v-else-if="eden.activeScreen === 'settings'"
            :settings="eden.codeToolsSettings"
            :vault-path="eden.vaultPath"
            :active-space="eden.activeSpace"
            :initial-tab="settingsInitialTab"
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
            :on-open-type-settings="openTypeSettings"
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
      </DesktopContentSurface>
    </DesktopChrome>

    <main v-else class="app-main app-main--zen">
      <div
        v-if="eden.isHydratingVault && eden.activeScreen !== 'settings' && eden.activeScreen !== 'object-types' && !eden.currentEntry"
        class="app-main-loading"
      >
        Загрузка данных...
      </div>
      <ObjectTypesSettings
        v-else-if="eden.activeScreen === 'object-types'"
        :note-types="eden.noteTypes"
        :initial-selected-type-id="eden.activeNoteTypeId"
        :create-draft-token="objectTypeCreateToken"
        :on-note-type-save="eden.saveNoteType"
        :on-note-type-delete="eden.deleteNoteType"
        @selected-type-change="onSelectedTypeChange"
      />
      <TypeObjectsView
        v-else-if="eden.activeScreen === 'type-collection' && activeCollectionType"
        :note-type="activeCollectionType"
        :entries="eden.entries"
        @open-entry="(id) => eden.navigateTo(id)"
        @create-entry="eden.createNewEntry(activeCollectionType.id)"
        @edit-type="openTypeSettings(activeCollectionType.id)"
      />
      <SettingsPage
        v-else-if="eden.activeScreen === 'settings'"
        :settings="eden.codeToolsSettings"
        :vault-path="eden.vaultPath"
        :active-space="eden.activeSpace"
        :initial-tab="settingsInitialTab"
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
        :on-open-type-settings="openTypeSettings"
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

    <!-- Legacy layout disabled after shared DesktopChrome/DesktopContentSurface migration.
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
    -->
  </div>

  <CustomCaret />
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, shallowRef, watch } from "vue";
import {
  DesktopChrome,
  DesktopContentSurface,
  CustomCaret,
  TitlebarHistoryControls,
  type TitlebarPlatform,
} from "@kepler/visuals";
import { useEdenStore } from "@/store/eden";
import { useLayoutStore } from "@/store/layout";
import { useKeyboard } from "@/composables/useKeyboard";
import { usePlatform } from "@/composables/usePlatform";
import { useSearch } from "@/composables/useSearch";
import type { SpaceId } from "@/components/sidebar/types";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import Titlebar from "./Titlebar.vue";
import SearchOverlay from "@/components/SearchOverlay.vue";
import EdenSidebar from "@/components/sidebar/EdenSidebar.vue";
import Editor from "./Editor.vue";
import SpacesView from "@/components/spaces/SpacesView.vue";
import SettingsPage from "@/components/settings/SettingsPage.vue";
import ObjectTypesSettings from "@/components/settings/ObjectTypesSettings.vue";
import TypeObjectsView from "@/components/objects/TypeObjectsView.vue";
import "@/App.css";

const eden = useEdenStore();
const layout = useLayoutStore();

type EdenHistorySnapshot = {
  activeScreen: "notes" | "settings" | "object-types" | "type-collection";
  currentEntryId: string | null;
  activeSpace: SpaceId;
  activeNoteTypeId: string | null;
};

type SettingsTab = "general" | "trash" | "storage" | "connected-apps" | "spaces";
const MAX_NAVIGATION_HISTORY = 30;

usePlatform();
useKeyboard();
const { pendingQuery } = useSearch();
const backStack = shallowRef<EdenHistorySnapshot[]>([]);
const forwardStack = shallowRef<EdenHistorySnapshot[]>([]);
const historyReady = shallowRef(false);
const suppressHistoryRecording = shallowRef(false);
const settingsInitialTab = shallowRef<SettingsTab>("general");
const objectTypeCreateToken = shallowRef(0);

function appendHistorySnapshot(
  snapshots: EdenHistorySnapshot[],
  snapshot: EdenHistorySnapshot,
) {
  const next = [...snapshots, snapshot];
  if (next.length <= MAX_NAVIGATION_HISTORY) {
    return next;
  }
  return next.slice(next.length - MAX_NAVIGATION_HISTORY);
}

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
    eden.entries.map((entry) => [entry.id, getEntryDisplayTitle(entry.title, entry.header_props_json)]),
  ),
);

const chromePlatform = computed<TitlebarPlatform>(() => {
  if (navigator.platform.startsWith("Mac")) return "mac";
  if (navigator.platform.startsWith("Linux")) return "linux";
  return "windows";
});

const showSidebarChrome = computed(() => true);
const activeCollectionType = computed(
  () => eden.noteTypes.find((noteType) => noteType.id === eden.activeNoteTypeId) ?? null,
);
const currentHistorySnapshot = computed<EdenHistorySnapshot | null>(() => {
  if (eden.isInitializing || !eden.vaultPath) return null;

  return {
    activeScreen: eden.activeScreen,
    currentEntryId: eden.currentEntry?.id ?? null,
    activeSpace: eden.activeSpace,
    activeNoteTypeId: eden.activeNoteTypeId,
  };
});

const canGoBack = computed(() => backStack.value.length > 0);
const canGoForward = computed(() => forwardStack.value.length > 0);

function snapshotsEqual(a: EdenHistorySnapshot | null, b: EdenHistorySnapshot | null) {
  if (!a || !b) return a === b;

  return (
    a.activeScreen === b.activeScreen &&
    a.currentEntryId === b.currentEntryId &&
    a.activeSpace === b.activeSpace &&
    a.activeNoteTypeId === b.activeNoteTypeId
  );
}

function openSettingsTab(tab: SettingsTab = "general") {
  settingsInitialTab.value = tab;
  eden.activeScreen = "settings";
}

function openObjectTypes(noteTypeId: string | null = null) {
  eden.openObjectTypes(noteTypeId);
}

function createObjectType() {
  eden.activeNoteTypeId = null;
  objectTypeCreateToken.value += 1;
  eden.activeScreen = "object-types";
}

function openTypeSettings(noteTypeId: string) {
  openObjectTypes(noteTypeId);
}

function handleSidebarBack() {
  if (eden.activeScreen === "object-types") {
    eden.activeScreen = "settings";
    return;
  }

  if (eden.activeScreen === "settings") {
    eden.activeScreen = "notes";
  }
}

function onSelectedTypeChange(noteTypeId: string | null) {
  eden.activeNoteTypeId = noteTypeId;
}

async function applyHistorySnapshot(snapshot: EdenHistorySnapshot) {
  suppressHistoryRecording.value = true;

  try {
    eden.activeSpace = snapshot.activeSpace;
    eden.activeScreen = snapshot.activeScreen;
    eden.activeNoteTypeId = snapshot.activeNoteTypeId;

    if (!snapshot.currentEntryId) {
      eden.currentEntry = null;
      return;
    }

    const existingEntry = eden.entries.find((entry) => entry.id === snapshot.currentEntryId);
    if (existingEntry) {
      eden.currentEntry = existingEntry;
      return;
    }

    if (!window.api) {
      eden.currentEntry = null;
      return;
    }

    const loadedEntry = await window.api.loadEntry(snapshot.currentEntryId);
    eden.currentEntry = loadedEntry ?? null;
  } finally {
    await nextTick();
    suppressHistoryRecording.value = false;
  }
}

async function navigateBack() {
  const targetSnapshot = backStack.value.at(-1);
  const currentSnapshot = currentHistorySnapshot.value;
  if (!targetSnapshot || !currentSnapshot) return;

  backStack.value = backStack.value.slice(0, -1);
  forwardStack.value = appendHistorySnapshot(forwardStack.value, currentSnapshot);
  await applyHistorySnapshot(targetSnapshot);
}

async function navigateForward() {
  const targetSnapshot = forwardStack.value.at(-1);
  const currentSnapshot = currentHistorySnapshot.value;
  if (!targetSnapshot || !currentSnapshot) return;

  forwardStack.value = forwardStack.value.slice(0, -1);
  backStack.value = appendHistorySnapshot(backStack.value, currentSnapshot);
  await applyHistorySnapshot(targetSnapshot);
}

const commandUnsubscribers: Array<() => void> = [];

onMounted(() => {
  void eden.initApp();

  if (window.api?.onCommand) {
    commandUnsubscribers.push(
      window.api.onCommand("eden:cmd:note:create", () => {
        if (layout.isSearchOpen) layout.closeSearch();
        if (layout.isZenMode) layout.disableZenMode();
        void eden.createNewEntry();
      }),
      window.api.onCommand("eden:cmd:note:search", () => {
        if (layout.isZenMode) layout.disableZenMode();
        layout.openSearch();
      }),
    );
  }
});

onUnmounted(() => {
  while (commandUnsubscribers.length > 0) {
    const off = commandUnsubscribers.pop();
    try {
      off?.();
    } catch {
      // ignore — best-effort cleanup
    }
  }
});

watch(currentHistorySnapshot, (nextSnapshot, previousSnapshot) => {
  if (!nextSnapshot) return;

  if (!historyReady.value) {
    historyReady.value = true;
    return;
  }

  if (suppressHistoryRecording.value || !previousSnapshot || snapshotsEqual(nextSnapshot, previousSnapshot)) {
    return;
  }

  backStack.value = appendHistorySnapshot(backStack.value, previousSnapshot);
  forwardStack.value = [];
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
    if (eden.isInitializing || eden.isHydratingVault || !eden.vaultPath || eden.activeScreen !== "notes") return;
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
      eden.activeNoteTypeId = null;
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
  eden.activeNoteTypeId = null;
  eden.currentEntry = null;
}
</script>
