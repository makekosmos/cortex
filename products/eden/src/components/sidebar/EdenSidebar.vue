<template>
  <SettingsSidebar v-if="!hidden" tone="strong">
    <div class="eden-settings-sidebar">
      <div class="eden-settings-sidebar__title">{{ sidebarTitle }}</div>

      <div class="eden-settings-sidebar__scroll kosmos-scroll">
        <div v-if="topItems.length > 0" class="eden-settings-sidebar__group">
          <SettingsSidebarButton
            v-for="item in topItems"
            :key="item.id"
            :icon="item.icon"
            :label="item.label"
            :test-id="item.testId"
            icon-variant="plain"
            @click="item.onClick()"
          />
        </div>

        <div v-if="primaryItems.length > 0" class="eden-settings-sidebar__group">
          <SettingsSidebarButton
            v-for="item in primaryItems"
            :key="item.id"
            :icon="item.icon"
            :label="item.label"
            :active="item.active"
            :test-id="item.testId"
            :icon-variant="item.iconVariant"
            :icon-from="item.iconFrom"
            :icon-to="item.iconTo"
            @click="item.onClick()"
          />
        </div>

        <section
          v-for="group in projectGroups"
          :key="group.id"
          class="eden-settings-sidebar__group"
        >
          <div class="eden-settings-sidebar__header">
            <span>{{ group.label }}</span>
            <button
              v-if="group.action"
              type="button"
              class="eden-settings-sidebar__action"
              :title="group.action.label"
              :data-testid="group.action.testId"
              @click="group.action.onClick()"
            >
              <component :is="group.action.icon" :size="14" :stroke-width="2" />
            </button>
          </div>

          <SettingsSidebarButton
            v-for="item in group.items"
            :key="item.id"
            :icon="item.icon"
            :icon-image="item.iconImage"
            :label="item.label"
            :active="item.active"
            :test-id="item.testId"
            icon-variant="plain"
            @click="item.onClick()"
            @contextmenu="item.onContextMenu?.($event)"
          />
        </section>
      </div>

      <div v-if="footerItems.length > 0" class="eden-settings-sidebar__footer">
        <SettingsSidebarButton
          v-for="item in footerItems"
          :key="item.id"
          :icon="item.icon"
          :label="item.label"
          :active="item.active"
          :test-id="item.testId"
          icon-variant="plain"
          @click="item.onClick()"
        />
      </div>
    </div>
  </SettingsSidebar>
</template>

<script setup lang="ts">
import { computed, type Component } from "vue";
import { SettingsSidebar, SettingsSidebarButton } from "@kosmos/visuals";
import {
  ArrowLeft,
  Database,
  FileText,
  Globe,
  Keyboard,
  Plus,
  Search,
  Settings,
  Shapes,
  Trash2,
} from "@lucide/vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { getNoteTypeCollectionName } from "@/lib/typedNotes";
import { isSystemType } from "@/lib/systemTypes";
import { objectIconUri } from "@/lib/iconResolver";

type EdenScreen = "notes" | "settings" | "object-types" | "type-collection";
type SettingsTab = "general" | "trash" | "storage" | "vim" | "spaces";

interface SidebarActionItem {
  id: string;
  icon: Component;
  label: string;
  active?: boolean;
  testId?: string;
  iconVariant?: "tile" | "plain";
  iconFrom?: string;
  iconTo?: string;
  onClick: () => void;
}

interface SidebarListItem {
  id: string;
  icon: Component;
  iconImage?: string;
  label: string;
  active?: boolean;
  testId?: string;
  onClick: () => void;
  onContextMenu?: (event: MouseEvent) => void;
}

interface SidebarGroup {
  id: string;
  label: string;
  items: SidebarListItem[];
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
}>();

const emit = defineEmits<{
  createEntry: [];
  toggleSearch: [];
  openEntry: [entryId: string];
  entryContextMenu: [event: MouseEvent, entryId: string];
  openSettingsTab: [tab: SettingsTab];
  openObjectTypes: [];
  openObjectType: [noteTypeId: string];
  createObjectType: [];
  back: [];
}>();

const noteTypesById = computed(
  () =>
    new Map(
      props.noteTypes.map((noteType) => [noteType.id, noteType] satisfies [string, NoteType]),
    ),
);

const systemNoteTypes = computed(() =>
  props.noteTypes.filter((noteType) => isSystemType(noteType.id)),
);
const customNoteTypes = computed(() =>
  props.noteTypes.filter((noteType) => !isSystemType(noteType.id)),
);

function buildEntryItem(entry: Entry, testId: string): SidebarListItem {
  const noteType = entry.type_id ? (noteTypesById.value.get(entry.type_id) ?? null) : null;

  return {
    id: entry.id,
    icon: FileText,
    iconImage: objectIconUri(noteType?.icon ?? "document"),
    label: getEntryDisplayTitle(entry.title, entry.header_props_json),
    active: props.currentEntry?.id === entry.id,
    onClick: () => emit("openEntry", entry.id),
    onContextMenu: (event: MouseEvent) => emit("entryContextMenu", event, entry.id),
    testId,
  };
}

function buildTypeItem(noteType: NoteType, testId: string): SidebarListItem {
  return {
    id: noteType.id,
    icon: Shapes,
    iconImage: objectIconUri(noteType.icon),
    label: getNoteTypeCollectionName(noteType),
    active: props.selectedObjectTypeId === noteType.id,
    onClick: () => emit("openObjectType", noteType.id),
    testId,
  };
}

const sidebarTitle = computed(() => {
  if (props.activeScreen === "settings") return "Настройки";
  if (props.activeScreen === "object-types") return "Типы объектов";
  if (props.activeScreen === "type-collection") return "Коллекция";
  return "Eden";
});

const notesPrimaryItems = computed<SidebarActionItem[]>(() => [
  {
    id: "create-entry",
    icon: Plus,
    label: "Новая заметка",
    onClick: () => emit("createEntry"),
    testId: "sidebar-create-entry",
    iconFrom: "var(--settings-sidebar-icon-from)",
    iconTo: "var(--settings-sidebar-icon-to)",
  },
  {
    id: "search",
    icon: Search,
    label: "Поиск",
    active: !!props.isSearchOpen || !!props.searchQuery,
    onClick: () => emit("toggleSearch"),
    testId: "widget-link-search",
    iconFrom: "var(--accent)",
    iconTo: "color-mix(in srgb, var(--accent) 60%, var(--background))",
  },
]);

const settingsPrimaryItems = computed<SidebarActionItem[]>(() => [
  {
    id: "general",
    icon: Settings,
    label: "Общие",
    active: props.activeSettingsTab === "general",
    onClick: () => emit("openSettingsTab", "general"),
    testId: "settings-nav-general",
  },
  {
    id: "trash",
    icon: Trash2,
    label: "Корзина",
    active: props.activeSettingsTab === "trash",
    onClick: () => emit("openSettingsTab", "trash"),
    testId: "settings-nav-trash",
  },
  {
    id: "storage",
    icon: Database,
    label: "Хранилище",
    active: props.activeSettingsTab === "storage",
    onClick: () => emit("openSettingsTab", "storage"),
    testId: "settings-nav-storage",
  },
  {
    id: "vim",
    icon: Keyboard,
    label: "Vim",
    active: props.activeSettingsTab === "vim",
    onClick: () => emit("openSettingsTab", "vim"),
    testId: "settings-nav-vim",
  },
  {
    id: "spaces",
    icon: Globe,
    label: "Пространства",
    active: props.activeSettingsTab === "spaces",
    onClick: () => emit("openSettingsTab", "spaces"),
    testId: "settings-nav-spaces",
  },
  {
    id: "object-types",
    icon: Shapes,
    label: "Типы объектов",
    onClick: () => emit("openObjectTypes"),
    testId: "settings-nav-object-types",
  },
]);

const objectTypesPrimaryItems = computed<SidebarActionItem[]>(() => [
  {
    id: "create-object-type",
    icon: Plus,
    label: "Новый тип",
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
  if (props.activeScreen === "settings") return settingsPrimaryItems.value;
  if (props.activeScreen === "object-types") return objectTypesPrimaryItems.value;
  return notesPrimaryItems.value;
});

const recentItems = computed<SidebarListItem[]>(() =>
  props.recentEntries.map((entry) => buildEntryItem(entry, `recent-entry-${entry.id}`)),
);

const noteObjectTypeItems = computed<SidebarListItem[]>(() =>
  [...props.noteTypes]
    .sort((left, right) => left.name.localeCompare(right.name, "ru"))
    .map((noteType) => buildTypeItem(noteType, `note-type-${noteType.id}`)),
);

const objectTypesSystemItems = computed<SidebarListItem[]>(() =>
  systemNoteTypes.value.map((noteType) => buildTypeItem(noteType, `system-type-${noteType.id}`)),
);

const objectTypesCustomItems = computed<SidebarListItem[]>(() =>
  customNoteTypes.value.map((noteType) => buildTypeItem(noteType, `custom-type-${noteType.id}`)),
);

const projectGroups = computed<SidebarGroup[]>(() => {
  if (props.activeScreen === "object-types") {
    return [
      { id: "system-types", label: "Системные типы", items: objectTypesSystemItems.value },
      { id: "custom-types", label: "Пользовательские", items: objectTypesCustomItems.value },
    ].filter((group) => group.items.length > 0);
  }

  if (props.activeScreen === "notes" || props.activeScreen === "type-collection") {
    return [
      { id: "recent", label: "Недавние", items: recentItems.value },
      {
        id: "objects",
        label: "Объекты",
        items: noteObjectTypeItems.value,
        action: {
          icon: Plus,
          label: "Новый тип",
          testId: "sidebar-create-object-type",
          onClick: () => emit("createObjectType"),
        },
      },
    ].filter((group) => group.items.length > 0 || group.action);
  }

  return [];
});

const footerItems = computed<SidebarActionItem[]>(() => {
  if (props.activeScreen !== "notes" && props.activeScreen !== "type-collection") return [];

  return [
    {
      id: "settings",
      icon: Settings,
      label: "Настройки",
      active: false,
      onClick: () => emit("openSettingsTab", "general"),
      testId: "open-settings-btn",
    },
  ];
});
</script>

<style scoped>
.eden-settings-sidebar {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 16px;
  padding: 0 8px 12px;
}

.eden-settings-sidebar__title {
  padding: 12px 12px 0;
  color: var(--foreground);
  font-family: var(--font-sans);
  font-size: 13px;
  font-weight: 500;
  line-height: 1.4;
  -webkit-app-region: drag;
}

.eden-settings-sidebar__scroll {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 24px;
  margin-right: -8px;
  overflow-y: auto;
  padding-right: 8px;
}

.eden-settings-sidebar__group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.eden-settings-sidebar__header {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 28px;
  padding: 4px 2px 6px 10px;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  font-size: 11px;
  font-weight: 600;
  line-height: 1.2;
  text-transform: uppercase;
}

.eden-settings-sidebar__header span {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.eden-settings-sidebar__action {
  display: inline-flex;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 58%, transparent);
  -webkit-app-region: no-drag;
}

.eden-settings-sidebar__action:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.eden-settings-sidebar__footer {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
</style>
