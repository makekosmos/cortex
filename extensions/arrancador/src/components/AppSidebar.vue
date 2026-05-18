<script setup lang="ts">
// Sidebar Arrancador-extension'а — поверх `Sidebar` из `@kosmos/visuals`.
//
// active-state определяется матчем `route.path` против `to` каждого
// nav-item'а (Sidebar сам не подписан на router).
import { computed } from "vue";
import { useRoute } from "vue-router";
import {
  Gamepad2,
  LayoutGrid,
  Search,
  Save,
  BarChart3,
  Settings,
} from "lucide-vue-next";
import { Sidebar } from "@kosmos/visuals";
import type { SidebarNavItem } from "@kosmos/visuals";

const props = defineProps<{
  hidden: boolean;
}>();

const emit = defineEmits<{
  "update:hidden": [hidden: boolean];
}>();

const route = useRoute();

const items: Array<{ id: string; to: string; label: string; icon: SidebarNavItem["icon"] }> = [
  { id: "library", to: "/", label: "Библиотека", icon: Gamepad2 },
  { id: "catalogue", to: "/catalogue", label: "Каталог", icon: LayoutGrid },
  { id: "scan", to: "/scan", label: "Сканер", icon: Search },
  { id: "sqoba", to: "/sqoba", label: "SQOBA", icon: Save },
  { id: "stats", to: "/stats", label: "Статистика", icon: BarChart3 },
  { id: "settings", to: "/settings", label: "Настройки", icon: Settings },
];

const primaryItems = computed<SidebarNavItem[]>(() =>
  items.map((it) => ({
    id: it.id,
    icon: it.icon,
    to: it.to,
    label: it.label,
    active:
      it.to === "/"
        ? route.path === "/" || route.path.startsWith("/game/")
        : route.path === it.to || route.path.startsWith(`${it.to}/`),
    testId: `arrancador-sidebar-${it.id}`,
  })),
);
</script>

<template>
  <Sidebar
    :primary-items="primaryItems"
    :hidden="props.hidden"
    :show-toggle="false"
    :reserve-top-inset="false"
    :default-width="200"
    :min-width="160"
    :max-width="280"
    @update:hidden="(val) => emit('update:hidden', val)"
  />
</template>
