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
  <div
    v-else
    :class="['app-container', { 'focus-mode-active': layout.isZenMode, 'eden-docked': isDocked }]"
  >
    <SearchOverlay
      :is-open="layout.isSearchOpen"
      :query="pendingQuery"
      :results="layout.searchResults"
      :entry-titles="entryTitlesById"
      @query-change="pendingQuery = $event"
      @close="onSearchClose"
      @result-select="onResultSelect"
    />

    <DesktopChrome appearance="settings" class="h-screen w-screen" :platform="chromePlatform">
      <template v-if="!layout.isZenMode" #titlebar-leading>
        <TitlebarHistoryControls
          :back-disabled="!canGoBack"
          :forward-disabled="!canGoForward"
          back-title="Назад"
          forward-title="Вперёд"
          @back="navigateBack"
          @forward="navigateForward"
        />
      </template>

      <template v-if="!layout.isZenMode" #sidebar>
        <EdenSidebar
          :hidden="layout.widgetSidebarHidden"
          :is-search-open="layout.isSearchOpen"
          :search-query="layout.searchQuery"
          :recent-entries="recentSidebarEntries"
          :note-types="eden.noteTypes"
          :current-entry="eden.currentEntry"
          :active-screen="eden.activeScreen"
          :active-settings-tab="settingsInitialTab"
          :selected-object-type-id="eden.activeNoteTypeId"
          @toggle-search="layout.isSearchOpen = !layout.isSearchOpen"
          @create-entry="eden.createNewEntry()"
          @open-entry="(id) => eden.navigateTo(id)"
          @entry-context-menu="onEntryContextMenu"
          @open-settings-tab="openSettingsTab"
          @open-object-types="openObjectTypes()"
          @open-object-type="eden.openTypeCollection($event)"
          @create-object-type="createObjectType()"
          @back="handleSidebarBack"
        />
      </template>

      <DesktopContentSurface>
        <main class="app-main">
          <div
            v-if="
              eden.isHydratingVault &&
              eden.activeScreen !== 'settings' &&
              eden.activeScreen !== 'object-types' &&
              !eden.currentEntry
            "
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
            :vault-path="eden.vaultPath"
            :active-space="eden.activeSpace"
            :initial-tab="settingsInitialTab"
            @select-vault="eden.selectFolder()"
            @select-space="onSelectSpace"
            @refresh-data="eden.refreshData()"
          />
          <CmEditor
            v-else-if="eden.currentEntry && useCmEditorForCurrent"
            :key="eden.currentEntry.id"
            :entry="eden.currentEntry"
            :zen-mode="layout.isZenMode"
            :vim-mode="preferences.state.vimModeEnabled"
            :on-save="eden.handleSave"
            @exit-zen="layout.disableZenMode()"
            @close-entry="closeCurrentEntry"
            @set-zen-mode="setZenMode"
            @live-char-count="liveCharCount = $event"
          />
          <Editor
            v-else-if="eden.currentEntry"
            :key="eden.currentEntry.id"
            :entry="eden.currentEntry"
            :all-entries="eden.entries"
            :note-types="eden.noteTypes"
            :zen-mode="layout.isZenMode"
            :on-save="eden.handleSave"
            :on-navigate="eden.navigateTo"
            :on-open-type-settings="openTypeSettings"
            @exit-zen="layout.disableZenMode()"
            @live-char-count="liveCharCount = $event"
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

    <div
      v-if="layout.isZenMode && currentEntryCharCount !== null"
      class="eden-char-counter"
      :class="{ 'has-overlap': charCounterHasOverlap }"
      data-testid="eden-char-counter"
    >
      {{ currentEntryCharCount }} {{ pluralizeCharacters(currentEntryCharCount) }}
    </div>

    <ContextMenu
      :open="entryMenu.isOpen.value"
      :x="entryMenu.x.value"
      :y="entryMenu.y.value"
      @close="entryMenu.close"
    >
      <ContextMenuItem data-testid="eden-entry-delete" @click="onDeleteContextEntry">
        Удалить
      </ContextMenuItem>
    </ContextMenu>

    <ToastHost />
  </div>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, shallowRef, watch } from "vue";
import {
  ContextMenu,
  ContextMenuItem,
  DesktopChrome,
  DesktopContentSurface,
  TitlebarHistoryControls,
  ToastHost,
  type TitlebarPlatform,
  provideToastHost,
  useContextMenu,
} from "@kosmos/visuals";
import { useEdenStore } from "@/store/eden";
import { useLayoutStore } from "@/store/layout";
import { useKeyboard } from "@/composables/useKeyboard";
import { usePlatform } from "@/composables/usePlatform";
import { useSearch } from "@/composables/useSearch";
import { useCharCounter } from "@/composables/useCharCounter";
import { useDockedWidget } from "@/composables/useDockedWidget";
import { useNavigationHistory } from "@/composables/useNavigationHistory";
import type { SpaceId } from "@/components/sidebar/types";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { usePreferences } from "@/composables/usePreferences";
import { shouldUseCmEditor } from "@/editor-cm/cmGate";
import Titlebar from "./Titlebar.vue";
import SearchOverlay from "@/components/SearchOverlay.vue";
import EdenSidebar from "@/components/sidebar/EdenSidebar.vue";
// Editor.vue таскает TipTap + lowlight + все code-block grammars (~1.3MB
// gzipped). Lazy-load — основной bundle открывается быстрее, заметка-чанк
// подгружается при первом открытии заметки.
const Editor = defineAsyncComponent(() => import("./Editor.vue"));
const CmEditor = defineAsyncComponent(() => import("./editor-cm/CmEditor.vue"));
import SpacesView from "@/components/spaces/SpacesView.vue";
import SettingsPage from "@/components/settings/SettingsPage.vue";
import ObjectTypesSettings from "@/components/settings/ObjectTypesSettings.vue";
import TypeObjectsView from "@/components/objects/TypeObjectsView.vue";
import "@/App.css";

const eden = useEdenStore();
const layout = useLayoutStore();
const preferences = usePreferences();

const useCmEditorForCurrent = computed(
  () =>
    eden.currentEntry != null &&
    shouldUseCmEditor(
      preferences.state.cmEditorEnabled || preferences.state.vimModeEnabled,
      eden.currentEntry.content_json,
    ),
);
// Provide toast api на root уровне — useToast() из любого descendant'а
// (Editor.vue и т.д.) увидит его. ToastHost дальше в template только
// рендерит, не повторяет provide.
provideToastHost();

type SettingsTab = "general" | "trash" | "storage" | "vim" | "spaces";

usePlatform();
useKeyboard();
const { pendingQuery } = useSearch();

// ПКМ-меню на entries в sidebar — пункт «Удалить» soft-delete'ит запись.
const entryMenu = useContextMenu<string>();

function onEntryContextMenu(event: MouseEvent, entryId: string) {
  entryMenu.open(event, entryId);
}

// Docked-widget state — для CSS-маркера (.eden-docked) на app-container.
const { isDocked } = useDockedWidget();

// На Windows native double-click-on-titlebar разворачивает окно. В zen mode
// это не нужно (header и так скрыт, maximize не имеет UX смысла) — отключаем,
// чтобы случайный dblclick не вырывал из режима фокуса.
watch(
  () => layout.isZenMode,
  (isZen) => {
    void (
      window as unknown as {
        kepler?: { window?: { setMaximizable?: (v: boolean) => Promise<void> } };
      }
    ).kepler?.window?.setMaximizable?.(!isZen);
  },
  { immediate: true },
);

async function onDeleteContextEntry() {
  const id = entryMenu.payload.value;
  entryMenu.close();
  if (!id || !window.api) return;
  try {
    await window.api.deleteEntry(id);
    await eden.refreshData();
    if (eden.currentEntry?.id === id) {
      eden.currentEntry = null;
    }
  } catch (err) {
    console.error("[eden] deleteEntry failed:", err);
  }
}
const { canGoBack, canGoForward, navigateBack, navigateForward } = useNavigationHistory(eden);
const settingsInitialTab = shallowRef<SettingsTab>("general");
const objectTypeCreateToken = shallowRef(0);

function pickRecentEntries(entries: Entry[], limit: number) {
  const topEntries: Entry[] = [];

  for (const entry of entries) {
    let insertAt = topEntries.findIndex((candidate) => entry.updated_at > candidate.updated_at);

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

const recentSidebarEntries = computed(() => pickRecentEntries(eden.entries, 10));

const { liveCharCount, currentEntryCharCount, charCounterHasOverlap, pluralizeCharacters } =
  useCharCounter(eden, layout);

const entryTitlesById = computed<Record<string, string>>(() =>
  Object.fromEntries(
    eden.entries.map((entry) => [
      entry.id,
      getEntryDisplayTitle(entry.title, entry.header_props_json),
    ]),
  ),
);

const chromePlatform = computed<TitlebarPlatform>(() => {
  if (navigator.platform.startsWith("Mac")) return "mac";
  if (navigator.platform.startsWith("Linux")) return "linux";
  return "windows";
});

const activeCollectionType = computed(
  () => eden.noteTypes.find((noteType) => noteType.id === eden.activeNoteTypeId) ?? null,
);
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

function closeCurrentEntry() {
  eden.currentEntry = null;
}

function setZenMode(enabled: boolean) {
  if (enabled) {
    layout.enableZenMode();
    return;
  }

  layout.disableZenMode();
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
      window.api.onCommand("eden:cmd:note:open-today", () => {
        console.log("[eden] open-today command received");
        if (layout.isSearchOpen) layout.closeSearch();
        void (async () => {
          try {
            await eden.openTodayJournal();
            console.log("[eden] openTodayJournal ok, currentEntry:", eden.currentEntry?.id);
            // Команда открывает заметку как чистый текст без header'а — zen mode
            // скрывает title + тип, оставляет только редактор. Полное расширение
            // открывается обычным путём (Alt+Space → Eden) → zen mode off.
            layout.enableZenMode();
          } catch (err) {
            console.error("[eden] openTodayJournal failed:", err);
          }
        })();
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
    if (
      eden.isInitializing ||
      eden.isHydratingVault ||
      !eden.vaultPath ||
      eden.activeScreen !== "notes"
    )
      return;
    if (eden.activeSpace === "my-space" && !eden.currentEntry) {
      void eden.openMySpace();
    }
  },
);

watch([() => layout.isZenMode, () => eden.activeScreen], ([isZenMode, activeScreen]) => {
  if (!isZenMode) return;

  layout.closeSearch();

  // Auto-disable zen mode ТОЛЬКО при уходе с notes screen (в Настройки,
  // типы объектов и т.п.). Не дёргаем на transitions currentEntry
  // (null → noteB → noteA), потому что во время навигации между
  // заметками currentEntry кратковременно null'ится, что валило zen
  // mode мид-navigation и ломало dock-corner dblclick на следующей
  // странице. Если юзер сам не находится ни на каком entry в notes
  // screen — пусть смотрит пустой editor, Esc выйдет руками.
  if (activeScreen !== "notes") {
    layout.disableZenMode();
  }
});

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
