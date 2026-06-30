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
    :class="[
      'app-container',
      {
        'focus-mode-active': layout.isZenMode,
        'focus-titlebar-hover': focusTitlebarHovered,
        'eden-docked': isDocked,
      },
    ]"
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
      <template #titlebar-leading>
        <div class="inline-flex items-center gap-2 [-webkit-app-region:no-drag]">
          <button
            v-if="layout.isZenMode"
            type="button"
            class="eden-titlebar-button inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color,opacity] duration-[120ms] ease-in [-webkit-app-region:no-drag] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-(--foreground)"
            title="Выйти из фокуса"
            aria-label="Выйти из фокуса"
            data-testid="titlebar-focus-exit"
            @click="layout.disableZenMode()"
          >
            <PhPottedPlant :size="17" weight="duotone" />
          </button>
          <button
            v-else-if="layout.widgetSidebarHidden"
            type="button"
            class="eden-titlebar-button inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color,opacity] duration-[120ms] ease-in [-webkit-app-region:no-drag] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-(--foreground)"
            title="Показать сайдбар"
            aria-label="Показать сайдбар"
            aria-pressed="false"
            data-testid="titlebar-sidebar-toggle"
            @click="layout.toggleWidgetSidebar()"
          >
            <PanelLeftOpen :size="16" />
          </button>
          <TitlebarHistoryControls
            v-if="!layout.isZenMode"
            :back-disabled="!canGoBack"
            :forward-disabled="!canGoForward"
            back-title="Назад"
            forward-title="Вперёд"
            @back="navigateBack"
            @forward="navigateForward"
          />
          <button
            v-if="!layout.isZenMode"
            type="button"
            class="eden-titlebar-button inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color,opacity] duration-[120ms] ease-in [-webkit-app-region:no-drag] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-(--foreground)"
            :title="preferences.state.readerModeEnabled ? 'Режим чтеца' : 'Режим писателя'"
            :aria-label="
              preferences.state.readerModeEnabled
                ? 'Переключиться в режим писателя'
                : 'Переключиться в режим чтеца'
            "
            :aria-pressed="preferences.state.readerModeEnabled"
            data-testid="titlebar-reader-mode-toggle"
            @click="preferences.setReaderModeEnabled(!preferences.state.readerModeEnabled)"
          >
            <BookOpen v-if="preferences.state.readerModeEnabled" :size="16" />
            <Pencil v-else :size="16" />
          </button>
        </div>
      </template>

      <template #titlebar-center>
        <Transition name="eden-titlebar-page-title">
          <div
            v-if="showTitlebarPageTitle"
            :key="titlebarPageTitle"
            class="eden-titlebar-page-title"
            data-testid="titlebar-page-title"
          >
            <span
              v-if="showTitlebarPersonIcon"
              class="eden-titlebar-page-title__icon"
              aria-hidden="true"
            >
              <img v-if="titlebarPersonImageSrc" :src="titlebarPersonImageSrc" alt="" />
              <User v-else :size="13" />
            </span>
            <span class="eden-titlebar-page-title__text">{{ titlebarPageTitle }}</span>
          </div>
        </Transition>
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
          :selected-object-type-id="eden.activeNoteTypeId"
          :sidebar-width="layout.widgetSidebarWidth"
          :resizable="true"
          @toggle-search="layout.isSearchOpen = !layout.isSearchOpen"
          @create-entry="eden.createNewEntry()"
          @open-diary="eden.openTodayJournal()"
          @open-entry="(id) => eden.navigateTo(id)"
          @entry-context-menu="onEntryContextMenu"
          @open-settings="openSettings"
          @open-object-type="openObjectCollection"
          @sidebar-width-change="layout.setWidgetSidebarWidth($event)"
          @toggle-sidebar="layout.toggleWidgetSidebar()"
        />
      </template>

      <main class="app-main">
        <div v-if="eden.isHydratingVault && !eden.currentEntry" class="app-main-loading">
          Загрузка данных...
        </div>
        <TypeObjectsView
          v-else-if="isCollectionViewActive && activeCollectionType"
          :note-type="activeCollectionType"
          :entries="eden.entries"
          @open-entry="(id) => eden.navigateTo(id)"
          @create-entry="eden.createNewEntry(activeCollectionType.id)"
        />
        <template v-else-if="eden.currentEntry">
          <ImageObjectView
            v-if="activeCurrentType?.id === SYSTEM_TYPE_IMAGE_ID"
            :entry="eden.currentEntry"
            :note-type="activeCurrentType"
          />
          <TiptapEditor
            v-else
            :key="`tiptap:${eden.currentEntry.id}:${eden.currentEntry.type_id ?? 'note_obj'}`"
            :entry="eden.currentEntry"
            :all-entries="eden.entries"
            :note-types="eden.noteTypes"
            :zen-mode="layout.isZenMode"
            :reader-mode="preferences.state.readerModeEnabled"
            :body-loading="eden.loadingEntryId === eden.currentEntry.id"
            :on-save="eden.handleSave"
            :on-navigate="eden.navigateTo"
            @close-entry="closeCurrentEntry"
            @set-zen-mode="setZenMode"
            @entry-draft-change="eden.updateEntryDraft"
            @live-char-count="liveCharCount = $event"
            @title-out-of-view-change="noteTitleOutOfView = $event"
            @type-change="onCmTypeChange"
          />
        </template>
        <div v-else class="app-empty-editor" data-testid="empty-editor-state" aria-live="polite">
          Никакая страница не выбрана
        </div>
      </main>

      <div
        v-if="layout.isZenMode"
        class="eden-focus-titlebar-title"
        data-testid="focus-titlebar-title"
      >
        {{ currentTitlebarTitle }}
      </div>
    </DesktopChrome>

    <div
      v-if="layout.isZenMode && currentEntryCharCount !== null"
      class="eden-char-counter"
      :class="{ 'has-overlap': charCounterHasOverlap }"
      data-testid="eden-char-counter"
    >
      {{ currentEntryCharCount }}
      {{ pluralizeCharacters(currentEntryCharCount) }}
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
import {
  computed,
  defineAsyncComponent,
  onMounted,
  onUnmounted,
  ref,
  shallowRef,
  watch,
} from "vue";
import {
  ContextMenu,
  ContextMenuItem,
  DesktopChrome,
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
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { usePreferences } from "@/composables/usePreferences";
import Titlebar from "./Titlebar.vue";
import SearchOverlay from "@/components/SearchOverlay.vue";
import EdenSidebar from "@/components/sidebar/EdenSidebar.vue";
const TiptapEditor = defineAsyncComponent(() => import("./editor-tiptap/TiptapEditor.vue"));
import ImageObjectView from "@/components/objects/ImageObjectView.vue";
import TypeObjectsView from "@/components/objects/TypeObjectsView.vue";
import {
  SYSTEM_TYPE_COLLECTION_ID,
  SYSTEM_TYPE_IMAGE_ID,
  SYSTEM_TYPE_PERSON_ID,
} from "@/lib/systemTypes";
import { getResolvedNoteTypeField } from "@/lib/typedNotes";
import { resolveObjectImageSrc } from "@/lib/objectImages";
import { PhPottedPlant } from "@phosphor-icons/vue";
import { BookOpen, PanelLeftOpen, Pencil, User } from "@lucide/vue";
import "@/App.css";

const eden = useEdenStore();
const layout = useLayoutStore();
const preferences = usePreferences();

function mountedEditorKind(): "tiptap" | "spaces" | "other" {
  if (document.querySelector(".tiptap-editor-host")) return "tiptap";
  if (document.querySelector(".spaces-view")) return "spaces";
  return "other";
}

function currentEdenDebug() {
  const entry = eden.currentEntry;
  return {
    activeScreen: eden.activeScreen,
    activeSpace: eden.activeSpace,
    editorKind: mountedEditorKind(),
    entry: entry
      ? {
          id: entry.id,
          title: entry.title,
          typeId: entry.type_id,
          deletedAt: entry.deleted_at,
          contentHead: entry.content_json.slice(0, 500),
        }
      : null,
  };
}

(window as unknown as { __edenDebug?: () => unknown }).__edenDebug = currentEdenDebug;

// Provide toast api на root уровне — useToast() из любого descendant'а
// увидит его. ToastHost дальше в template только
// рендерит, не повторяет provide.
provideToastHost();

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

const TITLEBAR_SYMBOL_VISIBLE = "#f5f5f5";
const TITLEBAR_SYMBOL_HIDDEN = "#00000000";
const FOCUS_TITLEBAR_HOVER_HEIGHT = 56;
const TITLEBAR_REVEAL_DURATION_MS = 400;
const FOCUS_TITLEBAR_LISTENER_CAPTURE = true;
const focusTitlebarHovered = ref(false);
let titlebarSymbolOpacity = 1;
let titlebarSymbolAnimationFrame: number | null = null;
let focusWindowDragPointerId: number | null = null;
let focusTitlebarListenersInstalled = false;
let titlebarHoverUnsubscribe: (() => void) | null = null;

function clearTitlebarSymbolAnimation(): void {
  if (titlebarSymbolAnimationFrame === null) return;
  window.cancelAnimationFrame(titlebarSymbolAnimationFrame);
  titlebarSymbolAnimationFrame = null;
}

function titlebarSymbolColor(opacity: number): string {
  if (opacity <= 0) return TITLEBAR_SYMBOL_HIDDEN;
  const alpha = Math.round(Math.min(1, Math.max(0, opacity)) * 255)
    .toString(16)
    .padStart(2, "0");
  return `${TITLEBAR_SYMBOL_VISIBLE}${alpha}`;
}

function setTitlebarSymbolOpacity(opacity: number): void {
  const nextOpacity = Math.min(1, Math.max(0, opacity));
  if (Math.abs(titlebarSymbolOpacity - nextOpacity) < 0.01) return;

  titlebarSymbolOpacity = nextOpacity;
  void window.kepler?.window?.setTitlebarSymbolColor?.(titlebarSymbolColor(nextOpacity));
}

function restoreTitlebarSymbols(): void {
  clearTitlebarSymbolAnimation();
  titlebarSymbolOpacity = 1;
  void window.kepler?.window?.setTitlebarSymbolColor?.(TITLEBAR_SYMBOL_VISIBLE);
}

function animateTitlebarSymbolOpacity(targetOpacity: number): void {
  clearTitlebarSymbolAnimation();

  const startOpacity = titlebarSymbolOpacity;
  const delta = targetOpacity - startOpacity;
  if (Math.abs(delta) < 0.01) {
    setTitlebarSymbolOpacity(targetOpacity);
    return;
  }

  const startedAt = performance.now();

  const tick = (now: number) => {
    const progress = Math.min(1, (now - startedAt) / TITLEBAR_REVEAL_DURATION_MS);
    const eased = 1 - Math.pow(1 - progress, 3);
    setTitlebarSymbolOpacity(startOpacity + delta * eased);

    if (progress < 1) {
      titlebarSymbolAnimationFrame = window.requestAnimationFrame(tick);
      return;
    }

    titlebarSymbolAnimationFrame = null;
    setTitlebarSymbolOpacity(targetOpacity);
  };

  titlebarSymbolAnimationFrame = window.requestAnimationFrame(tick);
}

function showFocusTitlebarChrome(): void {
  if (!layout.isZenMode) return;
  setFocusTitlebarHovered(true);
}

function setFocusTitlebarHovered(hovered: boolean): void {
  if (!layout.isZenMode) {
    focusTitlebarHovered.value = false;
    return;
  }

  focusTitlebarHovered.value = hovered;
  animateTitlebarSymbolOpacity(hovered ? 1 : 0);
}

function syncFocusTitlebarChrome(event?: MouseEvent | PointerEvent): void {
  if (!layout.isZenMode) {
    setFocusTitlebarHovered(false);
    return;
  }

  if (event && event.clientY >= 0 && event.clientY <= FOCUS_TITLEBAR_HOVER_HEIGHT) {
    setFocusTitlebarHovered(true);
    return;
  }

  setFocusTitlebarHovered(false);
}

function hideFocusTitlebarChrome(): void {
  setFocusTitlebarHovered(false);
}

function isFocusTitlebarDragTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  const header = target.closest(".kosmos-desktop-chrome-settings__header");
  if (!header) return false;
  if (
    target.closest('button, a, input, textarea, select, [role="button"], [contenteditable="true"]')
  )
    return false;
  if (
    target.closest(
      ".kosmos-desktop-chrome-settings__header-left, .kosmos-desktop-chrome-settings__header-center, .kosmos-desktop-chrome-settings__header-right",
    )
  )
    return false;

  return target === header;
}

function beginFocusWindowDrag(event: PointerEvent): void {
  if (!layout.isZenMode || event.button !== 0 || !isFocusTitlebarDragTarget(event.target)) return;

  event.preventDefault();
  focusWindowDragPointerId = event.pointerId;
  setFocusTitlebarHovered(true);
  void window.kepler?.window?.beginManualDrag?.({
    screenX: event.screenX,
    screenY: event.screenY,
  });
}

function moveFocusWindowDrag(event: PointerEvent): void {
  if (focusWindowDragPointerId !== event.pointerId) return;
  event.preventDefault();
  void window.kepler?.window?.moveManualDrag?.({
    screenX: event.screenX,
    screenY: event.screenY,
  });
}

function endFocusWindowDrag(): void {
  if (focusWindowDragPointerId === null) return;
  focusWindowDragPointerId = null;
  void window.kepler?.window?.endManualDrag?.();
}

function handleFocusTitlebarPointerMove(event: PointerEvent): void {
  syncFocusTitlebarChrome(event);
  moveFocusWindowDrag(event);
}

function installFocusTitlebarListeners(): void {
  if (focusTitlebarListenersInstalled) return;
  focusTitlebarListenersInstalled = true;
  window.addEventListener("pointerdown", beginFocusWindowDrag, FOCUS_TITLEBAR_LISTENER_CAPTURE);
  window.addEventListener(
    "pointermove",
    handleFocusTitlebarPointerMove,
    FOCUS_TITLEBAR_LISTENER_CAPTURE,
  );
  window.addEventListener("mousemove", syncFocusTitlebarChrome, FOCUS_TITLEBAR_LISTENER_CAPTURE);
  window.addEventListener("pointerup", endFocusWindowDrag, FOCUS_TITLEBAR_LISTENER_CAPTURE);
  window.addEventListener("pointercancel", endFocusWindowDrag, FOCUS_TITLEBAR_LISTENER_CAPTURE);
  document.addEventListener("mouseleave", hideFocusTitlebarChrome);
  window.addEventListener("blur", hideFocusTitlebarChrome);
}

function removeFocusTitlebarListeners(): void {
  if (!focusTitlebarListenersInstalled) return;
  focusTitlebarListenersInstalled = false;
  window.removeEventListener("pointerdown", beginFocusWindowDrag, FOCUS_TITLEBAR_LISTENER_CAPTURE);
  window.removeEventListener(
    "pointermove",
    handleFocusTitlebarPointerMove,
    FOCUS_TITLEBAR_LISTENER_CAPTURE,
  );
  window.removeEventListener("mousemove", syncFocusTitlebarChrome, FOCUS_TITLEBAR_LISTENER_CAPTURE);
  window.removeEventListener("pointerup", endFocusWindowDrag, FOCUS_TITLEBAR_LISTENER_CAPTURE);
  window.removeEventListener("pointercancel", endFocusWindowDrag, FOCUS_TITLEBAR_LISTENER_CAPTURE);
  document.removeEventListener("mouseleave", hideFocusTitlebarChrome);
  window.removeEventListener("blur", hideFocusTitlebarChrome);
  endFocusWindowDrag();
  focusTitlebarHovered.value = false;
}

function syncWindowMaximizeAvailability(): void {
  void window.kepler?.window?.setMaximizable?.(true);
}

watch(
  () => layout.isZenMode,
  (isZenMode) => {
    syncWindowMaximizeAvailability();
    void window.kepler?.window?.setTitlebarHoverTracking?.(isZenMode, FOCUS_TITLEBAR_HOVER_HEIGHT);
    if (isZenMode) {
      installFocusTitlebarListeners();
      setFocusTitlebarHovered(false);
    } else {
      removeFocusTitlebarListeners();
      restoreTitlebarSymbols();
    }
  },
  { immediate: true },
);

watch(
  () => eden.activeScreen,
  () => {
    syncWindowMaximizeAvailability();
  },
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
const noteTitleOutOfView = shallowRef(false);

const recentSidebarEntries = computed(() => eden.entries);

function openObjectCollection(noteTypeId: string, collectionEntryId?: string): void {
  if (collectionEntryId) {
    void eden.navigateTo(collectionEntryId);
    return;
  }
  eden.openTypeCollection(noteTypeId);
}

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
const currentTitlebarTitle = computed(() =>
  eden.currentEntry
    ? getEntryDisplayTitle(eden.currentEntry.title, eden.currentEntry.header_props_json)
    : "Eden",
);

const chromePlatform = computed<TitlebarPlatform>(() => {
  if (navigator.platform.startsWith("Mac")) return "mac";
  if (navigator.platform.startsWith("Linux")) return "linux";
  return "windows";
});

const activeCurrentType = computed(() =>
  eden.currentEntry
    ? (eden.noteTypes.find((noteType) => noteType.id === eden.currentEntry?.type_id) ?? null)
    : null,
);
const currentHeaderProps = computed<Record<string, unknown>>(() => {
  if (!eden.currentEntry?.header_props_json) return {};

  try {
    const parsed = JSON.parse(eden.currentEntry.header_props_json) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
});
const currentCollectionTypeId = computed(() => {
  if (eden.currentEntry?.type_id !== SYSTEM_TYPE_COLLECTION_ID) return null;
  const objectTypeId = currentHeaderProps.value.object_type_id;
  return typeof objectTypeId === "string" && objectTypeId.trim() ? objectTypeId : null;
});
const activeCollectionType = computed(() => {
  const collectionTypeId = currentCollectionTypeId.value ?? eden.activeNoteTypeId;
  return eden.noteTypes.find((noteType) => noteType.id === collectionTypeId) ?? null;
});
const isCollectionViewActive = computed(
  () =>
    eden.activeScreen === "type-collection" ||
    eden.currentEntry?.type_id === SYSTEM_TYPE_COLLECTION_ID,
);
const entriesById = computed(() => new Map(eden.entries.map((entry) => [entry.id, entry])));
const isCurrentPersonEntry = computed(() => activeCurrentType.value?.id === SYSTEM_TYPE_PERSON_ID);
const titlebarPersonImageSrc = computed(() => {
  if (!isCurrentPersonEntry.value) return "";

  const imageFieldId = getResolvedNoteTypeField(activeCurrentType.value, "image")?.visible
    ? "image"
    : "photo";
  return resolveObjectImageSrc(currentHeaderProps.value[imageFieldId], entriesById.value);
});
const showTitlebarPersonIcon = computed(() => isCurrentPersonEntry.value);
const titlebarPageTitle = computed(() => {
  if (layout.isZenMode) return "";

  if (activeCurrentType.value?.id === SYSTEM_TYPE_IMAGE_ID || noteTitleOutOfView.value) {
    return currentTitlebarTitle.value;
  }

  return "";
});
const showTitlebarPageTitle = computed(() => titlebarPageTitle.value.length > 0);
async function openSettings() {
  layout.closeSearch();
  pendingQuery.value = "";
  layout.searchQuery = "";
  layout.searchResults = [];
  await window.kepler?.edenSettings?.open?.();
}

function closeCurrentEntry() {
  eden.currentEntry = null;
}

function onCmTypeChange(entry: Entry) {
  eden.updateEntryDraft(entry);
}

function setZenMode(enabled: boolean) {
  if (enabled) {
    layout.enableZenMode();
    return;
  }

  layout.disableZenMode();
}

const commandUnsubscribers: Array<() => void> = [];

onMounted(() => {
  void eden.initApp();
  titlebarHoverUnsubscribe =
    window.kepler?.window?.onTitlebarHoverChange?.(setFocusTitlebarHovered) ?? null;
  void window.kepler?.window?.setTitlebarHoverTracking?.(
    layout.isZenMode,
    FOCUS_TITLEBAR_HOVER_HEIGHT,
  );
  if (layout.isZenMode) installFocusTitlebarListeners();

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
  titlebarHoverUnsubscribe?.();
  titlebarHoverUnsubscribe = null;
  void window.kepler?.window?.setTitlebarHoverTracking?.(false, FOCUS_TITLEBAR_HOVER_HEIGHT);
  removeFocusTitlebarListeners();
  restoreTitlebarSymbols();

  while (commandUnsubscribers.length > 0) {
    const off = commandUnsubscribers.pop();
    try {
      off?.();
    } catch {
      // ignore — best-effort cleanup
    }
  }
});

watch(
  () => eden.currentEntry?.id,
  () => {
    noteTitleOutOfView.value = false;
  },
);

watch([() => layout.isZenMode, () => eden.activeScreen], ([isZenMode, activeScreen]) => {
  if (!isZenMode) return;

  layout.closeSearch();

  // Auto-disable zen mode only when the app genuinely leaves notes mode.
  // Не дёргаем на transitions currentEntry (null → noteB → noteA),
  // потому что во время навигации между заметками currentEntry
  // кратковременно null'ится, что валило zen mode мид-navigation и
  // ломало dock-corner dblclick на следующей странице. Если юзер
  // сам не находится ни на каком entry в notes screen — пусть
  // смотрит пустой editor, выйти можно кнопкой titlebar или явным
  // hotkey toggle.
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
  await eden.navigateTo(entryId);
  pendingQuery.value = "";
  layout.searchQuery = "";
  layout.searchResults = [];
  layout.isSearchOpen = false;
}
</script>
