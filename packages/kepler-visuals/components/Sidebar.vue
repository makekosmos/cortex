<script setup lang="ts">
import type { Component } from "vue";
import { computed } from "vue";
import { RouterLink } from "vue-router";
import { PanelLeftClose } from "lucide-vue-next";
import ResizableSidebar, {
  type SidebarConfig,
} from "./ResizableSidebar.vue";
import SidebarButton from "./SidebarButton.vue";

export interface SidebarNavItem {
  id: string;
  icon: Component;
  to?: string;
  label?: string;
}

export interface SidebarProjectItem {
  id: string;
  label: string;
  to: string;
  active?: boolean;
  colorClass?: string;
}

interface Props {
  primaryItems: SidebarNavItem[];
  projectItems?: SidebarProjectItem[];
  projectSectionLabel?: string;
  footerItems?: SidebarNavItem[];
  isMac?: boolean;
  toggleTitle?: string;
  defaultWidth?: number;
  minWidth?: number;
  maxWidth?: number;
  hiddenWidth?: number;
  toggleShortcut?: string;
  dragRegion?: boolean;
  initialConfig?: Partial<SidebarConfig>;
}

const props = withDefaults(defineProps<Props>(), {
  projectItems: () => [],
  projectSectionLabel: "Проекты",
  footerItems: () => [],
  isMac: false,
  toggleTitle: "Скрыть сайдбар (⌘B)",
  defaultWidth: 200,
  minWidth: 160,
  maxWidth: 320,
  hiddenWidth: 80,
  toggleShortcut: undefined,
  dragRegion: false,
  initialConfig: undefined,
});

const emit = defineEmits<{
  configChange: [config: SidebarConfig];
}>();

const hasProjects = computed(() => props.projectItems.length > 0);
</script>

<template>
  <ResizableSidebar
    :default-width="defaultWidth"
    :min-width="minWidth"
    :max-width="maxWidth"
    :hidden-width="hiddenWidth"
    :toggle-shortcut="toggleShortcut"
    :drag-region="dragRegion"
    :initial-config="initialConfig"
    @config-change="emit('configChange', $event)"
  >
    <template #default="{ toggle }">
      <aside class="kepler-sidebar-shell flex min-h-0 h-full flex-col justify-between p-2">
        <div v-if="isMac" class="flex items-center justify-end pb-3">
          <button
            type="button"
            class="kepler-sidebar-top-toggle rounded-lg p-1.5 text-white/40 transition-colors hover:bg-white/6 hover:text-white"
            :title="toggleTitle"
            @click="toggle"
          >
            <PanelLeftClose :size="16" />
          </button>
        </div>

        <div class="flex flex-1 flex-col overflow-y-auto gap-0.5">
          <SidebarButton
            v-for="item in primaryItems"
            :key="item.id"
            :icon="item.icon"
            :to="item.to"
            :label="item.label"
          />

          <template v-if="hasProjects">
            <div class="my-2 border-t border-(--border)" />
            <div
              class="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-(--muted-foreground)/60 select-none"
            >
              {{ projectSectionLabel }}
            </div>
            <RouterLink
              v-for="project in projectItems"
              :key="project.id"
              :to="project.to"
              :class="[
                'kepler-sidebar-project-link flex h-10 w-full cursor-pointer items-center gap-2.5 px-3 py-2 text-sm transition-colors',
                project.active
                  ? 'bg-(--surface) text-(--foreground) font-medium'
                  : 'text-(--muted-foreground) hover:bg-(--secondary) hover:text-(--foreground)',
              ]"
            >
              <span
                :class="[
                  'kepler-sidebar-project-dot block h-2.5 w-2.5 shrink-0 rounded-full',
                  project.colorClass ?? 'bg-(--muted-foreground)',
                ]"
              />
              <span class="truncate select-none">{{ project.label }}</span>
            </RouterLink>
          </template>
        </div>

        <div v-if="footerItems.length > 0" class="flex flex-col gap-2">
          <SidebarButton
            v-for="item in footerItems"
            :key="item.id"
            :icon="item.icon"
            :to="item.to"
            :label="item.label"
          />
        </div>
      </aside>
    </template>
  </ResizableSidebar>
</template>

<style scoped>
.kepler-sidebar-project-link {
  border-radius: calc(var(--radius) * 1.4);
  corner-shape: var(--corner-shape);
}
</style>
