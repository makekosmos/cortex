<template>
  <KosmosSidebar
    :primary-items="primaryItems"
    :project-items="projectItems"
    :project-section-label="projectSectionLabel"
    :secondary-project-items="secondaryProjectItems"
    :secondary-project-section-label="secondaryProjectSectionLabel"
    :footer-items="footerItems"
    :top-item="topItem"
    :is-mac="isMac"
    :show-toggle="false"
    :reserve-top-inset="false"
    :default-width="200"
    :min-width="160"
    :max-width="320"
    :hidden-width="80"
    :initial-config="initialConfig"
    :hidden="hidden"
    @config-change="emit('configChange', $event)"
    @update:hidden="emit('update:hidden', $event)"
  />
</template>

<script setup lang="ts">
import { computed } from "vue";
import {
  Sidebar as KosmosSidebar,
  type SidebarConfig,
  type SidebarNavItem,
  type SidebarProjectItem,
} from "@kosmos/visuals";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { getNoteTypeCollectionName } from "@/lib/typedNotes";
import { isSystemType } from "@/lib/systemTypes";
import { objectIconUri } from "@/lib/iconResolver";
import {
  BackIcon,
  GlobeIcon,
  PlusIcon,
  SearchIcon,
  SettingsIcon,
  ShapesIcon,
  StorageIcon,
  TrashIcon,
} from "./edenSidebarIcons";

type EdenScreen = "notes" | "settings" | "object-types" | "type-collection";
type SettingsTab = "general" | "trash" | "storage" | "spaces";

const props = defineProps<{
  hidden: boolean;
  initialConfig?: Partial<SidebarConfig>;
  isSearchOpen?: boolean;
  searchQuery: string;
  recentEntries: Entry[];
  allEntries: Entry[];
  noteTypes: NoteType[];
  currentEntry: Entry | null;
  activeScreen: EdenScreen;
  activeSettingsTab: SettingsTab;
  selectedObjectTypeId: string | null;
}>();

const emit = defineEmits<{
  configChange: [config: SidebarConfig];
  "update:hidden": [hidden: boolean];
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

const isMac = navigator.platform.startsWith("Mac");
const neutralEntryIconColor = "rgb(255 255 255 / 0.58)";

const noteTypesById = computed(
  () => new Map(props.noteTypes.map((noteType) => [noteType.id, noteType] satisfies [string, NoteType])),
);

const systemNoteTypes = computed(() => props.noteTypes.filter((noteType) => isSystemType(noteType.id)));
const customNoteTypes = computed(() => props.noteTypes.filter((noteType) => !isSystemType(noteType.id)));

function buildEntryItem(entry: Entry, testId: string): SidebarProjectItem {
  const noteType = entry.type_id ? noteTypesById.value.get(entry.type_id) ?? null : null;
  const iconName = noteType?.icon ?? "document";

  return {
    id: entry.id,
    label: getEntryDisplayTitle(entry.title, entry.header_props_json),
    active: props.currentEntry?.id === entry.id,
    color: "var(--sidebar-foreground)",
    iconColor: neutralEntryIconColor,
    iconSrc: objectIconUri(iconName),
    onClick: () => emit("openEntry", entry.id),
    onContextMenu: (event: MouseEvent) => emit("entryContextMenu", event, entry.id),
    testId,
  };
}

function buildTypeItem(noteType: NoteType, testId: string): SidebarProjectItem {
  return {
    id: noteType.id,
    label: getNoteTypeCollectionName(noteType),
    active: props.selectedObjectTypeId === noteType.id,
    color: noteType.color,
    iconColor: noteType.color,
    iconSrc: objectIconUri(noteType.icon),
    onClick: () => emit("openObjectType", noteType.id),
    testId,
  };
}

const notesPrimaryItems = computed<SidebarNavItem[]>(() => [
  {
    id: "create-entry",
    icon: PlusIcon,
    label: "Новая заметка",
    onClick: () => emit("createEntry"),
    testId: "sidebar-create-entry",
  },
  {
    id: "search",
    icon: SearchIcon,
    label: "Поиск",
    active: !!props.isSearchOpen || !!props.searchQuery,
    onClick: () => emit("toggleSearch"),
    testId: "widget-link-search",
  },
]);

const notesRecentItems = computed<SidebarProjectItem[]>(() =>
  props.recentEntries.map((entry) => buildEntryItem(entry, `recent-entry-${entry.id}`)),
);

const noteObjectTypeItems = computed<SidebarProjectItem[]>(() =>
  [...props.noteTypes]
    .sort((left, right) => left.name.localeCompare(right.name, "ru"))
    .map((noteType) => buildTypeItem(noteType, `note-type-${noteType.id}`)),
);

const settingsPrimaryItems = computed<SidebarNavItem[]>(() => [
  {
    id: "general",
    icon: SettingsIcon,
    label: "Общие",
    active: props.activeSettingsTab === "general",
    onClick: () => emit("openSettingsTab", "general"),
    testId: "settings-nav-general",
  },
  {
    id: "trash",
    icon: TrashIcon,
    label: "Корзина",
    active: props.activeSettingsTab === "trash",
    onClick: () => emit("openSettingsTab", "trash"),
    testId: "settings-nav-trash",
  },
  {
    id: "storage",
    icon: StorageIcon,
    label: "Хранилище",
    active: props.activeSettingsTab === "storage",
    onClick: () => emit("openSettingsTab", "storage"),
    testId: "settings-nav-storage",
  },
  {
    id: "spaces",
    icon: GlobeIcon,
    label: "Пространства",
    active: props.activeSettingsTab === "spaces",
    onClick: () => emit("openSettingsTab", "spaces"),
    testId: "settings-nav-spaces",
  },
  {
    id: "object-types",
    icon: ShapesIcon,
    label: "Типы объектов",
    onClick: () => emit("openObjectTypes"),
    testId: "settings-nav-object-types",
  },
]);

const objectTypesPrimaryItems = computed<SidebarNavItem[]>(() => [
  {
    id: "create-object-type",
    icon: PlusIcon,
    label: "Новый тип",
    onClick: () => emit("createObjectType"),
    testId: "object-types-create",
  },
]);

const objectTypesSystemItems = computed<SidebarProjectItem[]>(() =>
  systemNoteTypes.value.map((noteType) => buildTypeItem(noteType, `system-type-${noteType.id}`)),
);

const objectTypesCustomItems = computed<SidebarProjectItem[]>(() =>
  customNoteTypes.value.map((noteType) => buildTypeItem(noteType, `custom-type-${noteType.id}`)),
);

const primaryItems = computed<SidebarNavItem[]>(() => {
  if (props.activeScreen === "settings") {
    return settingsPrimaryItems.value;
  }

  if (props.activeScreen === "object-types") {
    return objectTypesPrimaryItems.value;
  }

  return notesPrimaryItems.value;
});

const projectItems = computed<SidebarProjectItem[]>(() => {
  if (props.activeScreen === "object-types") {
    return objectTypesSystemItems.value;
  }

  if (props.activeScreen === "notes" || props.activeScreen === "type-collection") {
    return notesRecentItems.value;
  }

  return [];
});

const secondaryProjectItems = computed<SidebarProjectItem[]>(() => {
  if (props.activeScreen === "object-types") {
    return objectTypesCustomItems.value;
  }

  if (props.activeScreen === "notes" || props.activeScreen === "type-collection") {
    return noteObjectTypeItems.value;
  }

  return [];
});

const projectSectionLabel = computed(() => {
  if (props.activeScreen === "object-types") {
    return "Системные типы";
  }

  return "Недавние";
});

const secondaryProjectSectionLabel = computed(() => {
  if (props.activeScreen === "object-types") {
    return "Пользовательские";
  }

  return "Объекты";
});

const footerItems = computed<SidebarNavItem[]>(() => {
  if (props.activeScreen !== "notes" && props.activeScreen !== "type-collection") {
    return [];
  }

  return [
    {
      id: "settings",
      icon: SettingsIcon,
      label: "Настройки",
      active: false,
      onClick: () => emit("openSettingsTab", "general"),
      testId: "open-settings-btn",
    },
  ];
});

const topItem = computed<SidebarNavItem | undefined>(() => {
  if (props.activeScreen === "settings") {
    return {
      id: "settings-back",
      icon: BackIcon,
      label: "Назад к заметкам",
      onClick: () => emit("back"),
      testId: "settings-nav-back",
    };
  }

  if (props.activeScreen === "object-types") {
    return {
      id: "object-types-back",
      icon: BackIcon,
      label: "Назад к настройкам",
      onClick: () => emit("back"),
      testId: "object-types-nav-back",
    };
  }

  return undefined;
});
</script>
