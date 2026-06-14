<template>
  <SettingsSidebar v-if="!hidden" tone="strong" :style="settingsSidebarStyle">
    <template #title-leading>
      <div
        class="inline-flex items-center gap-1 [-webkit-app-region:no-drag]"
        data-testid="sidebar-header"
      >
        <button
          type="button"
          :class="SIDEBAR_ICON_BUTTON_CLASS"
          title="Скрыть сайдбар"
          aria-label="Скрыть сайдбар"
          aria-pressed="true"
          data-testid="sidebar-titlebar-toggle"
          @click="emit('toggleSidebar')"
        >
          <PanelLeftClose :size="16" />
        </button>
        <button
          type="button"
          :class="SIDEBAR_ICON_BUTTON_CLASS"
          title="Настройки"
          aria-label="Настройки"
          :aria-pressed="props.activeScreen === 'settings' || props.activeScreen === 'object-types'"
          data-testid="sidebar-header-settings"
          @click="emit('openSettingsTab', 'general')"
        >
          <Settings :size="16" />
        </button>
        <button
          type="button"
          :class="SIDEBAR_ICON_BUTTON_CLASS"
          title="Поиск"
          aria-label="Поиск"
          :aria-pressed="!!props.isSearchOpen || !!props.searchQuery"
          data-testid="widget-link-search"
          @click="emit('toggleSearch')"
        >
          <Search :size="16" />
        </button>
        <button
          type="button"
          :class="SIDEBAR_ICON_BUTTON_CLASS"
          title="Новая заметка"
          aria-label="Новая заметка"
          data-testid="sidebar-create-entry"
          @click="emit('createEntry')"
        >
          <Plus :size="16" />
        </button>
      </div>
    </template>

    <div class="eden-sidebar-shell">
      <div v-if="primaryItems.length > 0" class="eden-sidebar-primary">
        <SettingsSidebarButton
          v-for="item in primaryItems"
          :key="item.id"
          :icon="item.icon"
          :label="item.label"
          :active="item.active"
          :test-id="item.testId"
          icon-variant="plain"
          @click="item.onClick()"
        />
      </div>

      <div v-if="sidebarGroups.length > 0" class="eden-sidebar-groups">
        <section
          v-for="group in sidebarGroups"
          :key="group.id"
          class="eden-sidebar-group"
          :class="{ 'eden-sidebar-group--recent': group.id === 'recent' }"
        >
          <div
            v-if="group.id !== 'recent' || group.action"
            class="flex shrink-0 items-center justify-between px-1"
          >
            <span class="text-[11px] font-medium text-[var(--muted-foreground)]">
              {{ group.label }}
            </span>
            <Button
              v-if="group.action"
              type="button"
              size="sm"
              variant="ghost"
              :data-testid="group.action.testId"
              @click="group.action.onClick()"
            >
              Новый
            </Button>
          </div>

          <div
            v-if="group.id === 'recent'"
            ref="recentListRef"
            class="eden-sidebar-recent-list kosmos-scroll"
            data-testid="sidebar-recent-virtual-list"
            :data-recent-count="sortedRecentEntries.length"
            :data-rendered-count="recentVirtualItems.length"
            :data-range-start="recentVirtualRange.start"
            :data-range-end="recentVirtualRange.end"
            @scroll="syncRecentViewport"
          >
            <div
              class="eden-sidebar-recent-list__spacer"
              :style="{ height: `${recentVirtualTotalHeight}px` }"
            >
              <template v-for="virtualItem in recentVirtualItems" :key="virtualItem.row.id">
                <div
                  v-if="virtualItem.row.kind === 'month'"
                  class="eden-sidebar-recent-month"
                  :style="{
                    height: `${virtualItem.height}px`,
                    transform: `translateY(${virtualItem.top}px)`,
                  }"
                >
                  {{ virtualItem.row.label }}
                </div>
                <RecentSidebarItem
                  v-else-if="virtualItem.item"
                  :icon="virtualItem.item.icon"
                  :label="virtualItem.item.label"
                  :meta="virtualItem.item.meta"
                  :preview="virtualItem.item.preview"
                  :updated-label="virtualItem.item.updatedLabel"
                  :active="virtualItem.item.active"
                  :test-id="virtualItem.item.testId"
                  :style="{
                    height: `${virtualItem.height}px`,
                    transform: `translateY(${virtualItem.top}px)`,
                  }"
                  @click="virtualItem.item.onClick()"
                  @contextmenu="virtualItem.item.onContextMenu?.($event)"
                />
              </template>
            </div>
          </div>
          <div v-else class="flex min-w-0 shrink-0 flex-col gap-1">
            <SettingsSidebarButton
              v-for="item in group.items ?? []"
              :key="item.id"
              :icon="item.icon"
              :label="item.label"
              :active="item.active"
              :test-id="item.testId"
              icon-variant="plain"
              @click="item.onClick()"
              @contextmenu="item.onContextMenu?.($event)"
            />
          </div>
        </section>
      </div>
    </div>

    <div
      class="eden-sidebar-resize-handle"
      data-testid="eden-sidebar-resize-handle"
      role="separator"
      aria-orientation="vertical"
      title="Изменить ширину сайдбара"
      @pointerdown="beginSidebarResize"
    >
      <span class="eden-sidebar-resize-handle__line" aria-hidden="true" />
    </div>
  </SettingsSidebar>

  <Modal
    :open="objectTypesModalOpen"
    title="Объекты"
    width="min(420px, 92vw)"
    @close="objectTypesModalOpen = false"
  >
    <div class="eden-sidebar-objects-modal__list" data-testid="sidebar-objects-modal">
      <SettingsSidebarButton
        v-for="item in noteObjectTypeItems"
        :key="item.id"
        :icon="item.icon"
        :label="item.label"
        :active="item.active"
        :test-id="`objects-modal-${item.testId}`"
        icon-variant="plain"
        @click="openObjectTypeFromModal(item)"
      />
    </div>

    <template #footer>
      <Button
        type="button"
        size="sm"
        variant="ghost"
        data-testid="objects-modal-create-type"
        @click="createObjectTypeFromModal"
      >
        Новый тип
      </Button>
    </template>
  </Modal>
</template>

<script setup lang="ts">
import { computed, h, nextTick, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { Button, Modal, SettingsSidebar, SettingsSidebarButton } from "@kosmos/visuals";
import {
  ArrowLeft,
  Keyboard,
  PanelLeftClose,
  Plus,
  Search,
  Settings,
  Shapes,
  Trash2,
} from "@lucide/vue";
import {
  PhBarbell,
  PhBookOpen,
  PhBooks,
  PhCalendar,
  PhFile,
  PhFileText,
  PhFolderSimple,
  PhGameController,
  PhHeartbeat,
  PhImage,
  PhPlanet,
  PhSparkle,
  PhUser,
} from "@phosphor-icons/vue";
import { readEntryMarkdown } from "@/editor-cm/content";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { getNoteTypeCollectionName } from "@/lib/typedNotes";
import { isSystemType } from "@/lib/systemTypes";
import RecentSidebarItem from "./RecentSidebarItem.vue";

type EdenScreen = "notes" | "settings" | "object-types" | "type-collection";
type SettingsTab = "general" | "trash" | "vim";

interface SidebarActionItem {
  id: string;
  icon: Component;
  label: string;
  active?: boolean;
  testId?: string;
  onClick: () => void;
}

interface SidebarListItem {
  id: string;
  icon: Component;
  label: string;
  meta?: string;
  preview?: string;
  updatedLabel?: string;
  active?: boolean;
  testId?: string;
  onClick: () => void;
  onContextMenu?: (event: MouseEvent) => void;
}

type RecentTimelineRow =
  | {
      kind: "month";
      id: string;
      label: string;
    }
  | {
      kind: "entry";
      id: string;
      entry: Entry;
    };

interface RecentVirtualRow {
  row: RecentTimelineRow;
  top: number;
  height: number;
  item?: SidebarListItem;
}

interface SidebarGroup {
  id: string;
  label: string;
  items?: SidebarListItem[];
  action?: {
    icon: Component;
    label: string;
    testId?: string;
    onClick: () => void;
  };
}

const props = defineProps<{
  hidden: boolean;
  isSearchOpen?: boolean;
  searchQuery: string;
  recentEntries: Entry[];
  noteTypes: NoteType[];
  currentEntry: Entry | null;
  activeScreen: EdenScreen;
  activeSettingsTab: SettingsTab;
  selectedObjectTypeId: string | null;
  sidebarWidth: number;
}>();

const emit = defineEmits<{
  createEntry: [];
  toggleSearch: [];
  toggleSidebar: [];
  openEntry: [entryId: string];
  entryContextMenu: [event: MouseEvent, entryId: string];
  openSettingsTab: [tab: SettingsTab];
  openObjectTypes: [];
  openObjectType: [noteTypeId: string];
  createObjectType: [];
  sidebarWidthChange: [width: number];
  back: [];
}>();

const SIDEBAR_MIN_WIDTH = 240;
const SIDEBAR_MAX_WIDTH = 420;
const RECENT_ITEM_HEIGHT = 72;
const RECENT_MONTH_HEADER_HEIGHT = 26;
const RECENT_ITEM_GAP = 4;
const RECENT_ITEM_STRIDE = RECENT_ITEM_HEIGHT + RECENT_ITEM_GAP;
const RECENT_LIST_OVERSCAN = 6;
const RECENT_LIST_INITIAL_VISIBLE_COUNT = 12;
const TYPE_ICON_COLOR_FALLBACK = "var(--accent)";
const ENTRY_ICON_COLOR = "var(--muted-foreground)";
const SIDEBAR_ICON_BUTTON_CLASS =
  "inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color,opacity] duration-[120ms] ease-in [-webkit-app-region:no-drag] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-(--foreground)";
const objectTypesModalOpen = ref(false);
const isResizingSidebar = ref(false);
const recentListRef = ref<HTMLElement | null>(null);
const recentScrollTop = ref(0);
const recentViewportHeight = ref(0);
let resizeStartX = 0;
let resizeStartWidth = 0;

const clampedSidebarWidth = computed(() => clampSidebarWidth(props.sidebarWidth));
const settingsSidebarStyle = computed(() => ({
  width: `${clampedSidebarWidth.value}px`,
  minWidth: `${clampedSidebarWidth.value}px`,
}));

const NOTE_TYPE_ICON_COMPONENTS: Record<string, Component> = {
  document: PhFileText,
  "document-text": PhFileText,
  page: PhFile,
  "game-controller": PhGameController,
  image: PhImage,
  barbell: PhBarbell,
  fitness: PhHeartbeat,
  book: PhBookOpen,
  calendar: PhCalendar,
  planet: PhPlanet,
  library: PhBooks,
  folder: PhFolderSimple,
  sparkles: PhSparkle,
  user: PhUser,
  person: PhUser,
};

const sortedNoteTypes = computed(() =>
  [...props.noteTypes].sort((left, right) => left.name.localeCompare(right.name, "ru")),
);

const systemNoteTypes = computed(() =>
  sortedNoteTypes.value.filter((noteType) => isSystemType(noteType.id)),
);
const customNoteTypes = computed(() =>
  sortedNoteTypes.value.filter((noteType) => !isSystemType(noteType.id)),
);
const noteTypesById = computed(
  () => new Map(props.noteTypes.map((noteType) => [noteType.id, noteType] as const)),
);

function phosphorSidebarIcon(icon: Component, color: string): Component {
  return {
    inheritAttrs: false,
    setup(_, { attrs }) {
      return () => h(icon, { ...attrs, size: 16, weight: "duotone", color });
    },
  };
}

function resolveNoteTypeIcon(noteType: NoteType | null | undefined, color: string): Component {
  const icon = noteType?.icon ? NOTE_TYPE_ICON_COMPONENTS[noteType.icon] : null;
  return phosphorSidebarIcon(icon ?? PhFileText, color);
}

function clampSidebarWidth(width: number): number {
  if (!Number.isFinite(width)) return 280;
  return Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, Math.round(width)));
}

function isSortedByUpdatedAtDesc(entries: Entry[]): boolean {
  for (let index = 1; index < entries.length; index += 1) {
    if (entries[index - 1].updated_at < entries[index].updated_at) return false;
  }

  return true;
}

function isSameMonth(left: Date, right: Date): boolean {
  return left.getFullYear() === right.getFullYear() && left.getMonth() === right.getMonth();
}

function previousMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth() - 1, 1);
}

function monthKey(timestamp: number): string {
  const date = new Date(timestamp);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
}

function capitalize(value: string): string {
  return value ? value[0].toUpperCase() + value.slice(1) : value;
}

function formatMonthLabel(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  if (isSameMonth(date, now)) return "В этом месяце";
  if (isSameMonth(date, previousMonth(now))) return "В прошлом месяце";

  return capitalize(
    new Intl.DateTimeFormat("ru-RU", {
      month: "long",
      year: date.getFullYear() === now.getFullYear() ? undefined : "numeric",
    }).format(date),
  );
}

function formatUpdatedLabel(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  if (date.toDateString() === now.toDateString()) return "сегодня";

  return new Intl.DateTimeFormat("ru-RU", {
    day: "numeric",
    month: "short",
  }).format(date);
}

function entryPreview(entry: Entry): string {
  const markdown = readEntryMarkdown(entry.content_json);
  return markdown
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/!\[[^\]]*]\([^)]*\)/g, " ")
    .replace(/\[[^\]]*]\([^)]*\)/g, " ")
    .replace(/[#>*_`~\-[\]()]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 140);
}

function recentRowHeight(row: RecentTimelineRow): number {
  return row.kind === "month" ? RECENT_MONTH_HEADER_HEIGHT : RECENT_ITEM_HEIGHT;
}

function findRecentRowIndex(metrics: RecentVirtualRow[], offset: number): number {
  let low = 0;
  let high = metrics.length - 1;
  let result = metrics.length;

  while (low <= high) {
    const middle = Math.floor((low + high) / 2);
    const metric = metrics[middle];
    if (metric.top + metric.height >= offset) {
      result = middle;
      high = middle - 1;
    } else {
      low = middle + 1;
    }
  }

  return result;
}

function buildEntryItem(entry: Entry, testId: string): SidebarListItem {
  const noteType = noteTypesById.value.get(entry.type_id);

  return {
    id: entry.id,
    icon: resolveNoteTypeIcon(noteType, ENTRY_ICON_COLOR),
    label: getEntryDisplayTitle(entry.title, entry.header_props_json),
    meta: noteType ? getNoteTypeCollectionName(noteType) : undefined,
    preview: entryPreview(entry),
    updatedLabel: formatUpdatedLabel(entry.updated_at),
    active: props.activeScreen === "notes" && props.currentEntry?.id === entry.id,
    onClick: () => emit("openEntry", entry.id),
    onContextMenu: (event: MouseEvent) => emit("entryContextMenu", event, entry.id),
    testId,
  };
}

function buildTypeItem(noteType: NoteType, testId: string): SidebarListItem {
  return {
    id: noteType.id,
    icon: resolveNoteTypeIcon(noteType, noteType.color ?? TYPE_ICON_COLOR_FALLBACK),
    label: getNoteTypeCollectionName(noteType),
    active:
      (props.activeScreen === "type-collection" || props.activeScreen === "object-types") &&
      props.selectedObjectTypeId === noteType.id,
    onClick: () => emit("openObjectType", noteType.id),
    testId,
  };
}

function openObjectTypeFromModal(item: SidebarListItem): void {
  objectTypesModalOpen.value = false;
  item.onClick();
}

function createObjectTypeFromModal(): void {
  objectTypesModalOpen.value = false;
  emit("createObjectType");
}

function beginSidebarResize(event: PointerEvent): void {
  if (event.button !== 0) return;

  event.preventDefault();
  isResizingSidebar.value = true;
  resizeStartX = event.clientX;
  resizeStartWidth = clampedSidebarWidth.value;
  window.addEventListener("pointermove", handleSidebarResize);
  window.addEventListener("pointerup", endSidebarResize);
  window.addEventListener("pointercancel", endSidebarResize);
}

function handleSidebarResize(event: PointerEvent): void {
  if (!isResizingSidebar.value) return;
  emit("sidebarWidthChange", clampSidebarWidth(resizeStartWidth + event.clientX - resizeStartX));
}

function endSidebarResize(): void {
  if (!isResizingSidebar.value) return;
  isResizingSidebar.value = false;
  window.removeEventListener("pointermove", handleSidebarResize);
  window.removeEventListener("pointerup", endSidebarResize);
  window.removeEventListener("pointercancel", endSidebarResize);
}

function syncRecentViewport(event?: Event): void {
  const element =
    event?.currentTarget instanceof HTMLElement ? event.currentTarget : recentListRef.value;
  if (!element) return;

  const scrollTop = Number(element.scrollTop);
  const clientHeight = Number(element.clientHeight);

  recentScrollTop.value = Number.isFinite(scrollTop) ? scrollTop : 0;
  recentViewportHeight.value = Number.isFinite(clientHeight) ? clientHeight : 0;
}

onBeforeUnmount(() => {
  endSidebarResize();
  window.removeEventListener("resize", syncRecentViewport);
});

onMounted(() => {
  window.addEventListener("resize", syncRecentViewport);
  void nextTick(syncRecentViewport);
});

const objectTypesLauncherItem = computed<SidebarActionItem | null>(() => {
  if (props.activeScreen !== "notes" && props.activeScreen !== "type-collection") return null;

  return {
    id: "objects",
    icon: Shapes,
    label: "Объекты",
    active: props.activeScreen === "type-collection",
    onClick: () => {
      objectTypesModalOpen.value = true;
    },
    testId: "sidebar-open-objects",
  };
});

const settingsPrimaryItems = computed<SidebarActionItem[]>(() => [
  {
    id: "general",
    icon: Settings,
    label: "Общие",
    active: props.activeScreen === "settings" && props.activeSettingsTab === "general",
    onClick: () => emit("openSettingsTab", "general"),
    testId: "settings-nav-general",
  },
  {
    id: "trash",
    icon: Trash2,
    label: "Корзина",
    active: props.activeScreen === "settings" && props.activeSettingsTab === "trash",
    onClick: () => emit("openSettingsTab", "trash"),
    testId: "settings-nav-trash",
  },
  {
    id: "vim",
    icon: Keyboard,
    label: "Vim",
    active: props.activeScreen === "settings" && props.activeSettingsTab === "vim",
    onClick: () => emit("openSettingsTab", "vim"),
    testId: "settings-nav-vim",
  },
  {
    id: "object-types",
    icon: Shapes,
    label: "Типы объектов",
    active: props.activeScreen === "object-types",
    onClick: () => emit("openObjectTypes"),
    testId: "settings-nav-object-types",
  },
]);

const objectTypesPrimaryItems = computed<SidebarActionItem[]>(() => [
  {
    id: "create-object-type",
    icon: Plus,
    label: "Новый тип",
    active: props.activeScreen === "object-types" && props.selectedObjectTypeId === null,
    onClick: () => emit("createObjectType"),
    testId: "object-types-create",
  },
]);

const topItems = computed<SidebarActionItem[]>(() => {
  if (props.activeScreen === "settings") {
    return [
      {
        id: "settings-back",
        icon: ArrowLeft,
        label: "Назад к заметкам",
        onClick: () => emit("back"),
        testId: "settings-nav-back",
      },
    ];
  }

  if (props.activeScreen === "object-types") {
    return [
      {
        id: "object-types-back",
        icon: ArrowLeft,
        label: "Назад к настройкам",
        onClick: () => emit("back"),
        testId: "object-types-nav-back",
      },
    ];
  }

  return [];
});

const primaryItems = computed<SidebarActionItem[]>(() => {
  if (props.activeScreen === "settings") return [...topItems.value, ...settingsPrimaryItems.value];
  if (props.activeScreen === "object-types") {
    return [...topItems.value, ...objectTypesPrimaryItems.value];
  }
  return objectTypesLauncherItem.value ? [objectTypesLauncherItem.value] : [];
});

const sortedRecentEntries = computed<Entry[]>(() => {
  const currentEntry = props.currentEntry;
  const entries =
    currentEntry && !props.recentEntries.some((entry) => entry.id === currentEntry.id)
      ? [currentEntry, ...props.recentEntries]
      : props.recentEntries;

  return isSortedByUpdatedAtDesc(entries)
    ? entries
    : [...entries].sort((left, right) => right.updated_at - left.updated_at);
});

const recentTimelineRows = computed<RecentTimelineRow[]>(() => {
  const rows: RecentTimelineRow[] = [];
  let activeMonthKey = "";

  for (const entry of sortedRecentEntries.value) {
    const nextMonthKey = monthKey(entry.updated_at);
    if (nextMonthKey !== activeMonthKey) {
      activeMonthKey = nextMonthKey;
      rows.push({
        kind: "month",
        id: `month-${nextMonthKey}`,
        label: formatMonthLabel(entry.updated_at),
      });
    }

    rows.push({
      kind: "entry",
      id: `entry-${entry.id}`,
      entry,
    });
  }

  return rows;
});

const recentTimelineMetrics = computed<RecentVirtualRow[]>(() => {
  let top = 0;

  return recentTimelineRows.value.map((row) => {
    const height = recentRowHeight(row);
    const metric: RecentVirtualRow = { row, top, height };
    top += height + RECENT_ITEM_GAP;
    return metric;
  });
});

const recentVirtualTotalHeight = computed(() => {
  const last = recentTimelineMetrics.value.at(-1);
  return last ? last.top + last.height : 0;
});

const recentVirtualRange = computed(() => {
  const metrics = recentTimelineMetrics.value;
  const count = metrics.length;
  if (count === 0) return { start: 0, end: 0 };

  const viewportHeight =
    recentViewportHeight.value || RECENT_ITEM_STRIDE * RECENT_LIST_INITIAL_VISIBLE_COUNT;
  const visibleStart = findRecentRowIndex(metrics, recentScrollTop.value);
  const visibleEnd = findRecentRowIndex(metrics, recentScrollTop.value + viewportHeight);
  const start = Math.max(0, visibleStart - RECENT_LIST_OVERSCAN);
  const end = Math.min(count, visibleEnd + RECENT_LIST_OVERSCAN + 1);
  return { start, end };
});

const recentVirtualItems = computed(() =>
  recentTimelineMetrics.value
    .slice(recentVirtualRange.value.start, recentVirtualRange.value.end)
    .map((virtualRow) => {
      return {
        ...virtualRow,
        item:
          virtualRow.row.kind === "entry"
            ? buildEntryItem(virtualRow.row.entry, `recent-entry-${virtualRow.row.entry.id}`)
            : undefined,
      };
    }),
);

watch(
  () => sortedRecentEntries.value.length,
  () => {
    void nextTick(syncRecentViewport);
  },
);

const noteObjectTypeItems = computed<SidebarListItem[]>(() =>
  sortedNoteTypes.value.map((noteType) => buildTypeItem(noteType, `note-type-${noteType.id}`)),
);

const objectTypesSystemItems = computed<SidebarListItem[]>(() =>
  systemNoteTypes.value.map((noteType) => buildTypeItem(noteType, `system-type-${noteType.id}`)),
);

const objectTypesCustomItems = computed<SidebarListItem[]>(() =>
  customNoteTypes.value.map((noteType) => buildTypeItem(noteType, `custom-type-${noteType.id}`)),
);

const sidebarGroups = computed<SidebarGroup[]>(() => {
  if (props.activeScreen === "object-types") {
    return [
      { id: "system-types", label: "Системные типы", items: objectTypesSystemItems.value },
      { id: "custom-types", label: "Пользовательские", items: objectTypesCustomItems.value },
    ].filter((group) => group.items?.length);
  }

  if (props.activeScreen === "notes" || props.activeScreen === "type-collection") {
    return sortedRecentEntries.value.length > 0 ? [{ id: "recent", label: "Недавние" }] : [];
  }

  return [];
});
</script>

<style scoped>
.eden-sidebar-shell {
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 1.5rem;
  padding: 0.25rem 0 0.5rem;
  overflow: hidden;
}

.eden-sidebar-groups {
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 1.5rem;
}

.eden-sidebar-primary {
  display: flex;
  min-width: 0;
  flex-shrink: 0;
  flex-direction: column;
  gap: 0.25rem;
  padding: 0 0.5rem;
}

.eden-sidebar-group {
  display: flex;
  min-width: 0;
  flex: 0 0 auto;
  flex-direction: column;
  gap: 0.5rem;
}

.eden-sidebar-group--recent {
  min-height: 0;
  flex: 1 1 auto;
}

.eden-sidebar-objects-modal__list {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.25rem;
}

.eden-sidebar-recent-list {
  position: relative;
  min-width: 0;
  height: max(160px, calc(100vh - 152px));
  min-height: 0;
  flex: 0 1 auto;
  overflow-x: hidden;
  overflow-y: auto;
  scrollbar-gutter: stable both-edges;
  overscroll-behavior: contain;
}

.eden-sidebar-recent-list__spacer {
  position: relative;
  min-width: 0;
}

.eden-sidebar-recent-month {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  display: flex;
  align-items: flex-end;
  padding: 0 0.5rem 0.25rem;
  color: color-mix(in srgb, var(--foreground) 46%, transparent);
  font-family: var(--font-sans);
  font-size: 11px;
  font-weight: 650;
  letter-spacing: 0;
  box-shadow: inset 0 -1px color-mix(in srgb, var(--foreground) 7%, transparent);
}

.eden-sidebar-recent-list :deep(.eden-recent-sidebar-item) {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 72px;
}

.eden-sidebar-resize-handle {
  position: absolute;
  top: 0;
  right: -6px;
  z-index: 80;
  display: flex;
  width: 12px;
  height: 100%;
  align-items: center;
  justify-content: center;
  cursor: col-resize;
  -webkit-app-region: no-drag;
}

.eden-sidebar-resize-handle__line {
  width: 4px;
  height: 32px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--foreground) 22%, transparent);
  opacity: 0;
  transition:
    opacity 120ms ease,
    background-color 120ms ease;
}

.eden-sidebar-resize-handle:hover .eden-sidebar-resize-handle__line {
  opacity: 1;
  background: color-mix(in srgb, var(--foreground) 36%, transparent);
}

:deep(.kosmos-settings-sidebar-button:hover) {
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
}

:deep(.kosmos-settings-sidebar-button[data-active="true"]) {
  background: var(--settings-sidebar-active);
}

:deep(.kosmos-settings-sidebar-button[data-active="true"]:hover) {
  background: var(--settings-sidebar-active);
}
</style>
