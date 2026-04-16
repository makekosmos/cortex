<script setup lang="ts">
import type { Component } from "vue";
import { computed } from "vue";
import { RouterLink } from "vue-router";
import { PanelLeftClose } from "lucide-vue-next";
import ResizableSidebar, { type SidebarConfig } from "./ResizableSidebar.vue";
import SidebarButton from "./SidebarButton.vue";

export interface SidebarNavItem {
  id: string;
  icon: Component;
  to?: string;
  label?: string;
  active?: boolean;
  testId?: string;
  onClick?: () => void;
}

export interface SidebarProjectItem {
  id: string;
  label: string;
  to?: string;
  active?: boolean;
  colorClass?: string;
  color?: string;
  testId?: string;
  onClick?: () => void;
}

interface Props {
  primaryItems: SidebarNavItem[];
  projectItems?: SidebarProjectItem[];
  projectSectionLabel?: string;
  footerItems?: SidebarNavItem[];
  topItem?: SidebarNavItem;
  isMac?: boolean;
  className?: string;
  hidden?: boolean;
  offsetX?: number;
  toggleTitle?: string;
  defaultWidth?: number;
  minWidth?: number;
  maxWidth?: number;
  hiddenWidth?: number;
  toggleShortcut?: string;
  dragRegion?: boolean;
  initialConfig?: Partial<SidebarConfig>;
  showToggle?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  projectItems: () => [],
  projectSectionLabel: "Проекты",
  footerItems: () => [],
  topItem: undefined,
  isMac: false,
  className: undefined,
  hidden: undefined,
  offsetX: 0,
  toggleTitle: "Скрыть сайдбар (⌘B)",
  defaultWidth: 200,
  minWidth: 160,
  maxWidth: 320,
  hiddenWidth: 80,
  toggleShortcut: undefined,
  dragRegion: false,
  initialConfig: undefined,
  showToggle: true,
});

const emit = defineEmits<{
  configChange: [config: SidebarConfig];
  "update:hidden": [hidden: boolean];
}>();

const hasProjects = computed(() => props.projectItems.length > 0);
const shellClasses = computed(() => [
  "kepler-sidebar-shell",
  { "kepler-sidebar-shell--mac": props.isMac },
  { "kepler-sidebar-shell--with-toggle": props.showToggle },
]);
</script>

<template>
  <ResizableSidebar
    :default-width="defaultWidth"
    :min-width="minWidth"
    :max-width="maxWidth"
    :hidden-width="hiddenWidth"
    :toggle-shortcut="toggleShortcut"
    :class-name="className"
    :drag-region="dragRegion"
    :offset-x="offsetX"
    :hidden="hidden"
    :initial-config="initialConfig"
    @config-change="emit('configChange', $event)"
    @update:hidden="emit('update:hidden', $event)"
  >
    <template #default="{ toggle }">
      <aside :class="shellClasses">
        <div class="kepler-sidebar-top">
          <button
            v-if="props.topItem"
            type="button"
            class="kepler-sidebar-top-toggle"
            :data-testid="props.topItem.testId"
            :title="props.topItem.label"
            @click="'onClick' in props.topItem && typeof props.topItem.onClick === 'function' ? props.topItem.onClick() : undefined"
          >
            <component :is="props.topItem.icon" :size="16" />
          </button>
          <button
            v-if="props.showToggle"
            type="button"
            class="kepler-sidebar-top-toggle"
            data-testid="sidebar-toggle"
            :title="toggleTitle"
            @click="toggle"
          >
            <PanelLeftClose :size="16" />
          </button>
        </div>

        <div class="kepler-sidebar-body">
          <SidebarButton
            v-for="item in primaryItems"
            :key="item.id"
            :icon="item.icon"
            :to="item.to"
            :label="item.label"
            :active="item.active"
            :test-id="item.testId"
            @click="'onClick' in item && typeof item.onClick === 'function' ? item.onClick() : undefined"
          />

          <template v-if="hasProjects">
            <div class="kepler-sidebar-divider" />
            <div class="kepler-sidebar-section-label">
              {{ projectSectionLabel }}
            </div>
            <template v-for="project in projectItems" :key="project.id">
              <RouterLink
                v-if="project.to"
                :to="project.to"
                :data-testid="project.testId"
                :class="[
                  'kepler-sidebar-project-link',
                  'widget-nav-item',
                  project.active
                    ? 'kepler-sidebar-project-link--active'
                    : '',
                ]"
              >
                <span
                  :class="[
                    'kepler-sidebar-project-dot',
                    project.colorClass ?? 'bg-(--muted-foreground)',
                  ]"
                  :style="project.color ? { backgroundColor: project.color } : undefined"
                />
                <span class="kepler-sidebar-project-label">{{ project.label }}</span>
              </RouterLink>
              <button
                v-else
                type="button"
                :data-testid="project.testId"
                :class="[
                  'kepler-sidebar-project-link',
                  'widget-nav-item',
                  project.active
                    ? 'kepler-sidebar-project-link--active'
                    : '',
                ]"
                @click="'onClick' in project && typeof project.onClick === 'function' ? project.onClick() : undefined"
              >
                <span
                  :class="[
                    'kepler-sidebar-project-dot',
                    project.colorClass ?? 'bg-(--muted-foreground)',
                  ]"
                  :style="project.color ? { backgroundColor: project.color } : undefined"
                />
                <span class="kepler-sidebar-project-label">{{ project.label }}</span>
              </button>
            </template>
          </template>
        </div>

        <div v-if="footerItems.length > 0" class="kepler-sidebar-footer">
          <SidebarButton
            v-for="item in footerItems"
            :key="item.id"
            :icon="item.icon"
            :to="item.to"
            :label="item.label"
            :active="item.active"
            :test-id="item.testId"
            @click="'onClick' in item && typeof item.onClick === 'function' ? item.onClick() : undefined"
          />
        </div>
      </aside>
    </template>
  </ResizableSidebar>
</template>

<style scoped>
.kepler-sidebar-shell {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  min-height: 0;
  height: 100%;
  padding: 0.5rem;
}

.kepler-sidebar-shell--mac {
  padding-top: var(--kepler-mac-sidebar-top-safe-area, 52px);
}

.kepler-sidebar-shell--mac.kepler-sidebar-shell--with-toggle {
  padding-top: 0.5rem;
}

.kepler-sidebar-top {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding-bottom: 0.75rem;
}

.kepler-sidebar-shell--mac .kepler-sidebar-top {
  position: absolute;
  top: 12px;
  left: auto;
  right: 0.5rem;
  min-height: 28px;
  justify-content: flex-end;
  padding-bottom: 0;
  z-index: 1;
}

.kepler-sidebar-top-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0.375rem;
  border-radius: 0.5rem;
  color: color-mix(in srgb, var(--sidebar-foreground) 40%, transparent);
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kepler-sidebar-top-toggle:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 6%, transparent);
  color: var(--sidebar-foreground);
}

.kepler-sidebar-body {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  gap: 0.125rem;
  overflow-y: auto;
}

.kepler-sidebar-shell--mac.kepler-sidebar-shell--with-toggle .kepler-sidebar-body {
  padding-top: 2.5rem;
}

.kepler-sidebar-divider {
  margin: 0.5rem 0;
  border-top: 1px solid var(--border);
}

.kepler-sidebar-section-label {
  padding: 0.5rem 0.75rem 0.25rem;
  color: color-mix(in srgb, var(--muted-foreground) 60%, transparent);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  user-select: none;
}

.kepler-sidebar-project-link {
  display: flex;
  align-items: center;
  gap: 0.625rem;
  width: 100%;
  min-height: 2.5rem;
  padding: 0.5rem 0.75rem;
  border-radius: calc(var(--radius) * 1.4);
  corner-shape: var(--corner-shape);
  color: var(--muted-foreground);
  cursor: pointer;
  text-decoration: none;
  text-align: left;
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kepler-sidebar-project-link:hover {
  background: var(--secondary);
  color: var(--foreground);
}

.kepler-sidebar-project-link--active {
  background: var(--surface);
  color: var(--foreground);
  font-weight: 500;
}

.kepler-sidebar-project-dot {
  display: block;
  width: 0.625rem;
  height: 0.625rem;
  flex-shrink: 0;
  border-radius: 999px;
}

.kepler-sidebar-project-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  user-select: none;
}

.kepler-sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
</style>
