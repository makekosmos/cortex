<template>
  <SettingsSidebar v-if="!hidden" tone="strong" :title="sidebarTitle">
    <template #title-leading>
      <button
        type="button"
        class="inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color,opacity] duration-[120ms] ease-in [-webkit-app-region:no-drag] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-(--foreground)"
        title="Скрыть сайдбар"
        aria-label="Скрыть сайдбар"
        aria-pressed="true"
        data-testid="sidebar-titlebar-toggle"
        @click="emit('toggleSidebar')"
      >
        <PanelLeftClose :size="16" />
      </button>
    </template>

    <div class="flex min-h-0 flex-1 flex-col gap-6 px-2 pb-2">
      <div v-if="primaryItems.length > 0" class="flex flex-col gap-1">
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

      <div v-if="sidebarGroups.length > 0" class="flex min-h-0 flex-1 flex-col gap-6">
        <section v-for="group in sidebarGroups" :key="group.id" class="flex min-h-0 flex-col gap-2">
          <div class="flex items-center justify-between px-1">
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

          <div class="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto kosmos-scroll">
            <SettingsSidebarButton
              v-for="item in group.items"
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

      <div v-if="footerItems.length > 0" class="mt-auto flex flex-col gap-1">
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
import { Button, SettingsSidebar, SettingsSidebarButton } from "@kosmos/visuals";
import {
  ArrowLeft,
  FileText,
  Keyboard,
  PanelLeftClose,
  Plus,
  Search,
  Settings,
  Shapes,
  Trash2,
} from "@lucide/vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { getNoteTypeCollectionName } from "@/lib/typedNotes";
import { isSystemType } from "@/lib/systemTypes";

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
  toggleSidebar: [];
  openEntry: [entryId: string];
  entryContextMenu: [event: MouseEvent, entryId: string];
  openSettingsTab: [tab: SettingsTab];
  openObjectTypes: [];
  openObjectType: [noteTypeId: string];
  createObjectType: [];
  back: [];
}>();

const systemNoteTypes = computed(() =>
  props.noteTypes.filter((noteType) => isSystemType(noteType.id)),
);
const customNoteTypes = computed(() =>
  props.noteTypes.filter((noteType) => !isSystemType(noteType.id)),
);

function buildEntryItem(entry: Entry, testId: string): SidebarListItem {
  return {
    id: entry.id,
    icon: FileText,
    label: getEntryDisplayTitle(entry.title, entry.header_props_json),
    active: props.activeScreen === "notes" && props.currentEntry?.id === entry.id,
    onClick: () => emit("openEntry", entry.id),
    onContextMenu: (event: MouseEvent) => emit("entryContextMenu", event, entry.id),
    testId,
  };
}

function buildTypeItem(noteType: NoteType, testId: string): SidebarListItem {
  return {
    id: noteType.id,
    icon: Shapes,
    label: getNoteTypeCollectionName(noteType),
    active:
      (props.activeScreen === "type-collection" || props.activeScreen === "object-types") &&
      props.selectedObjectTypeId === noteType.id,
    onClick: () => emit("openObjectType", noteType.id),
    testId,
  };
}

const notesPrimaryItems = computed<SidebarActionItem[]>(() => [
  {
    id: "create-entry",
    icon: Plus,
    label: "Новая заметка",
    onClick: () => emit("createEntry"),
    testId: "sidebar-create-entry",
  },
  {
    id: "search",
    icon: Search,
    label: "Поиск",
    active: !!props.isSearchOpen || !!props.searchQuery,
    onClick: () => emit("toggleSearch"),
    testId: "widget-link-search",
  },
]);

const sidebarTitle = computed(() => {
  if (props.activeScreen === "settings") return "Настройки";
  if (props.activeScreen === "object-types") return "Типы объектов";
  if (props.activeScreen === "type-collection") return "Коллекция";
  return "Eden";
});

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
    id: "vim",
    icon: Keyboard,
    label: "Vim",
    active: props.activeSettingsTab === "vim",
    onClick: () => emit("openSettingsTab", "vim"),
    testId: "settings-nav-vim",
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
  if (props.activeScreen === "settings") return [...topItems.value, ...settingsPrimaryItems.value];
  if (props.activeScreen === "object-types") {
    return [...topItems.value, ...objectTypesPrimaryItems.value];
  }
  return notesPrimaryItems.value;
});

const recentItems = computed<SidebarListItem[]>(() => {
  const entries =
    props.currentEntry && !props.recentEntries.some((entry) => entry.id === props.currentEntry?.id)
      ? [props.currentEntry, ...props.recentEntries]
      : props.recentEntries;

  return entries.map((entry) => buildEntryItem(entry, `recent-entry-${entry.id}`));
});

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

const sidebarGroups = computed<SidebarGroup[]>(() => {
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
      active: props.activeScreen === "settings",
      onClick: () => emit("openSettingsTab", "general"),
      testId: "open-settings-btn",
    },
  ];
});
</script>
