<script setup lang="ts">
import { computed } from "vue";
import { RouterLink, useRoute } from "vue-router";
import {
  Archive,
  Book,
  Calendar,
  Circle,
  Inbox,
  PanelLeftClose,
  Settings,
  Star,
} from "lucide-vue-next";
import SideBarButton from "@/components/SideBarButton.vue";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { ProjectStatus } from "@/types/task";
import ResizableSidebar from "@kepler/visuals/components/ResizableSidebar.vue";
import type { SidebarConfig } from "@kepler/visuals/components/ResizableSidebar.vue";

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
}

function colorTagClass(colorTag?: string | null): string {
  switch (colorTag) {
    case "red":
      return "text-red-500";
    case "orange":
      return "text-orange-500";
    case "yellow":
      return "text-yellow-500";
    case "green":
      return "text-green-500";
    case "blue":
      return "text-blue-500";
    case "purple":
      return "text-purple-500";
    case "pink":
      return "text-pink-500";
    default:
      return "text-(--muted-foreground)";
  }
}

const store = useTodoStore();
const { projects } = storeToRefs(store);
const route = useRoute();

const activeProjects = computed(() =>
  projects.value
    .filter((p) => p.status === ProjectStatus.Active)
    .sort((a, b) => a.sortOrder - b.sortOrder),
);
</script>

<template>
  <ResizableSidebar
    :default-width="200"
    :min-width="160"
    :max-width="320"
    :hidden-width="80"
    toggle-shortcut="meta+b"
    drag-region
    :initial-config="loadConfig()"
    @config-change="saveConfig"
  >
    <template #default="{ toggle }">
      <aside class="flex min-h-0 h-full flex-col justify-between p-2">
        <div class="flex items-center justify-end">
          <button
            type="button"
            class="rounded-lg p-1.5 text-white/40 hover:bg-white/6 hover:text-white transition-colors"
            title="Скрыть сайдбар (⌘B)"
            @click="toggle"
          >
            <PanelLeftClose :size="16" />
          </button>
        </div>

        <div class="flex flex-1 flex-col overflow-y-auto">
          <SideBarButton :icon="Inbox" to="/" label="Входящие" />
          <SideBarButton :icon="Star" to="/today" label="Сегодня" />
          <SideBarButton :icon="Calendar" to="/upcoming" label="Планы" />
          <SideBarButton :icon="Book" to="/logbook" label="Журнал" />
          <SideBarButton :icon="Archive" to="/trash" label="Корзина" />

          <template v-if="activeProjects.length > 0">
            <div class="my-2 border-t border-(--border)" />
            <div
              class="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-(--muted-foreground)/60 select-none"
            >
              Проекты
            </div>
            <RouterLink
              v-for="project in activeProjects"
              :key="project.id"
              :to="`/project/${project.id}`"
              :class="[
                'flex h-10 w-full cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors',
                route.path === `/project/${project.id}`
                  ? 'bg-(--accent) text-(--foreground) font-medium'
                  : 'text-(--muted-foreground) hover:bg-(--secondary) hover:text-(--foreground)',
              ]"
            >
              <Circle
                :size="10"
                :class="[
                  'shrink-0 fill-current',
                  colorTagClass(project.colorTag),
                ]"
              />
              <span class="truncate">{{ project.title }}</span>
            </RouterLink>
          </template>
        </div>

        <div class="flex flex-col gap-2">
          <SideBarButton :icon="Settings" to="/settings" />
        </div>
      </aside>
    </template>
  </ResizableSidebar>
</template>
