<script setup lang="ts">
import { computed } from "vue";
import {
  Archive,
  Book,
  Calendar,
  CalendarDays,
  Inbox,
  Kanban,
  Settings,
  Star,
} from "lucide-vue-next";
import {
  Sidebar as KeplerSidebar,
  type SidebarConfig,
  type SidebarNavItem,
  type SidebarProjectItem,
} from "@kepler/visuals";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { ProjectStatus } from "@/types/task";
import { setSidebarHidden } from "@/composables/useSidebarState";
import { useRoute } from "vue-router";

const STORAGE_KEY = "delphi-sidebar-config";

function loadConfig(): Partial<SidebarConfig> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw) as Partial<SidebarConfig>;
  } catch {
    // ignore
  }
  return {};
}

function saveConfig(config: SidebarConfig) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
  setSidebarHidden(config.hidden);
}

function colorTagClass(colorTag?: string | null): string {
  switch (colorTag) {
    case "red":
      return "bg-red-500";
    case "orange":
      return "bg-orange-500";
    case "yellow":
      return "bg-yellow-500";
    case "green":
      return "bg-green-500";
    case "blue":
      return "bg-blue-500";
    case "purple":
      return "bg-purple-500";
    case "pink":
      return "bg-pink-500";
    default:
      return "bg-(--muted-foreground)";
  }
}

const store = useTodoStore();
const { projects } = storeToRefs(store);
const route = useRoute();

const isMac = navigator.platform.startsWith("Mac");

const primaryItems = computed<SidebarNavItem[]>(() => [
  { id: "inbox", icon: Inbox, to: "/", label: "Входящие" },
  { id: "today", icon: Star, to: "/today", label: "Сегодня" },
  { id: "upcoming", icon: Calendar, to: "/upcoming", label: "Планы" },
  { id: "calendar", icon: CalendarDays, to: "/calendar", label: "Календарь" },
  { id: "week", icon: Kanban, to: "/week", label: "Неделя" },
  { id: "logbook", icon: Book, to: "/logbook", label: "Журнал" },
  { id: "trash", icon: Archive, to: "/trash", label: "Корзина" },
]);

const footerItems = computed<SidebarNavItem[]>(() => [
  { id: "settings", icon: Settings, to: "/settings" },
]);

const projectItems = computed<SidebarProjectItem[]>(() =>
  projects.value
    .filter((project) => project.status === ProjectStatus.Active)
    .sort((a, b) => a.sortOrder - b.sortOrder)
    .map((project) => ({
      id: project.id,
      label: project.title,
      to: `/project/${project.id}`,
      active: route.path === `/project/${project.id}`,
      colorClass: colorTagClass(project.colorTag),
    })),
);
</script>

<template>
  <KeplerSidebar
    :primary-items="primaryItems"
    :project-items="projectItems"
    :footer-items="footerItems"
    :is-mac="isMac"
    :default-width="200"
    :min-width="160"
    :max-width="320"
    :hidden-width="80"
    toggle-shortcut="meta+b|ctrl+b"
    drag-region
    :initial-config="loadConfig()"
    @config-change="saveConfig"
  />
</template>
