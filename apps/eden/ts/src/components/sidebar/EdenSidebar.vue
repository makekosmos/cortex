<template>
  <KeplerSidebar
    :primary-items="primaryItems"
    :project-items="recentItems"
    project-section-label="Недавние"
    :footer-items="footerItems"
    :is-mac="isMac"
    :default-width="200"
    :min-width="160"
    :max-width="320"
    :hidden-width="80"
    toggle-shortcut="meta+b|ctrl+b"
    drag-region
    :initial-config="initialConfig"
    :hidden="hidden"
    @config-change="emit('configChange', $event)"
    @update:hidden="emit('update:hidden', $event)"
  />
</template>

<script setup lang="ts">
import { computed } from "vue";
import {
  Sidebar as KeplerSidebar,
  type SidebarConfig,
  type SidebarNavItem,
  type SidebarProjectItem,
} from "@kepler/visuals";
import { PlusIcon, SearchIcon, SettingsIcon } from "./edenSidebarIcons";

const props = defineProps<{
  hidden: boolean;
  initialConfig?: Partial<SidebarConfig>;
  isSearchOpen?: boolean;
  searchQuery: string;
  recentEntries: Entry[];
  currentEntry: Entry | null;
}>();

const emit = defineEmits<{
  configChange: [config: SidebarConfig];
  "update:hidden": [hidden: boolean];
  createEntry: [];
  toggleSearch: [];
  openSettings: [];
  openEntry: [entryId: string];
}>();

const isMac = navigator.platform.startsWith("Mac");

const primaryItems = computed<SidebarNavItem[]>(() => [
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

const recentItems = computed<SidebarProjectItem[]>(() =>
  props.recentEntries.map((entry) => ({
    id: entry.id,
    label: entry.title.trim() || "Без названия",
    active: props.currentEntry?.id === entry.id,
    color: entry.type_id ? "var(--accent)" : "var(--muted-foreground)",
    onClick: () => emit("openEntry", entry.id),
    testId: `recent-entry-${entry.id}`,
  })),
);

const footerItems = computed<SidebarNavItem[]>(() => [
  {
    id: "settings",
    icon: SettingsIcon,
    label: "Настройки",
    onClick: () => emit("openSettings"),
    testId: "open-settings-btn",
  },
]);
</script>
