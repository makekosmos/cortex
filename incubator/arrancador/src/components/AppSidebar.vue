<script setup lang="ts">
import type { Component } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  BarChart3,
  Gamepad2,
  LayoutGrid,
  PanelLeftClose,
  Save,
  Search,
  Settings,
} from "@lucide/vue";
import { SettingsSidebar, SettingsSidebarButton } from "@kosmos/visuals";

const props = defineProps<{
  hidden: boolean;
}>();

const emit = defineEmits<{
  "update:hidden": [hidden: boolean];
}>();

const route = useRoute();
const router = useRouter();

interface SidebarItem {
  id: string;
  to: string;
  label: string;
  icon: Component;
}

const primaryItems: SidebarItem[] = [
  { id: "library", to: "/", label: "Библиотека", icon: Gamepad2 },
  { id: "catalogue", to: "/catalogue", label: "Каталог", icon: LayoutGrid },
  { id: "scan", to: "/scan", label: "Сканер", icon: Search },
  { id: "sqoba", to: "/sqoba", label: "SQOBA", icon: Save },
  { id: "stats", to: "/stats", label: "Статистика", icon: BarChart3 },
];

const footerItems: SidebarItem[] = [
  { id: "settings", to: "/settings", label: "Настройки", icon: Settings },
];

function isActive(item: SidebarItem): boolean {
  return item.to === "/"
    ? route.path === "/" || route.path.startsWith("/game/")
    : route.path === item.to || route.path.startsWith(`${item.to}/`);
}

function go(to: string) {
  void router.push(to);
}
</script>

<template>
  <SettingsSidebar v-if="!props.hidden" tone="strong">
    <template #title-leading>
      <div class="arrancador-sidebar__titlebar" data-testid="arrancador-sidebar-header">
        <button
          type="button"
          class="arrancador-sidebar__icon-button"
          title="Скрыть сайдбар"
          aria-label="Скрыть сайдбар"
          data-testid="arrancador-sidebar-titlebar-toggle"
          @click="emit('update:hidden', true)"
        >
          <PanelLeftClose :size="16" />
        </button>
        <SettingsSidebarButton
          :icon="Gamepad2"
          label="Arrancador"
          icon-variant="plain"
          :active="route.path === '/'"
          test-id="arrancador-sidebar-home"
          @click="go('/')"
        />
      </div>
    </template>

    <div class="arrancador-sidebar">
      <div class="arrancador-sidebar__section">
        <SettingsSidebarButton
          v-for="item in primaryItems"
          :key="item.id"
          :icon="item.icon"
          :label="item.label"
          :active="isActive(item)"
          :test-id="`arrancador-sidebar-${item.id}`"
          icon-variant="plain"
          @click="go(item.to)"
        />
      </div>

      <div class="arrancador-sidebar__footer">
        <SettingsSidebarButton
          v-for="item in footerItems"
          :key="item.id"
          :icon="item.icon"
          :label="item.label"
          :active="isActive(item)"
          :test-id="`arrancador-sidebar-${item.id}`"
          icon-variant="plain"
          @click="go(item.to)"
        />
      </div>
    </div>
  </SettingsSidebar>
</template>

<style scoped>
.arrancador-sidebar__titlebar {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  align-items: center;
  gap: 4px;
}

.arrancador-sidebar__titlebar :deep(.kosmos-settings-sidebar-button) {
  min-width: 0;
  flex: 1 1 auto;
}

.arrancador-sidebar__icon-button {
  display: inline-flex;
  width: var(--kosmos-titlebar-control-size, 32px);
  height: var(--kosmos-titlebar-control-size, 32px);
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border-radius: var(--kosmos-titlebar-control-radius, 8px);
  color: color-mix(in srgb, var(--sidebar-foreground) 72%, transparent);
  transition:
    background-color 120ms ease,
    color 120ms ease;
  -webkit-app-region: no-drag;
}

.arrancador-sidebar__icon-button:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.arrancador-sidebar {
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 16px;
  padding: 4px 8px 8px;
}

.arrancador-sidebar__section,
.arrancador-sidebar__footer {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.arrancador-sidebar__footer {
  margin-top: auto;
}
</style>
