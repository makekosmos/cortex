<script setup lang="ts">
import { computed, onMounted } from "vue";
import { DesktopChrome, DesktopContentSurface } from "@kosmos/visuals";
import SidebarItem from "../dashboard/SidebarItem.vue";
import ObjectTable from "../dashboard/ObjectTable.vue";
import {
  currentTypeId,
  loadObjects,
  loadObjectTypes,
  objects,
  objectsLoading,
  objectTypes,
} from "../dashboard/store";

const TYPE_COLORS = [
  "#ef4444",
  "#f97316",
  "#eab308",
  "#22c55e",
  "#06b6d4",
  "#3b82f6",
  "#a855f7",
  "#ec4899",
];

function colorForType(id: string): string {
  let hash = 0;
  for (let i = 0; i < id.length; i++) hash = (hash * 31 + id.charCodeAt(i)) >>> 0;
  return TYPE_COLORS[hash % TYPE_COLORS.length];
}

const showSettings = computed(() => currentTypeId.value === "__settings__");

const titleLabel = computed(() => {
  if (showSettings.value) return "Настройки";
  if (currentTypeId.value === null) return "Всё";
  const t = objectTypes.value.find((x) => x.id === currentTypeId.value);
  return t?.name ?? currentTypeId.value;
});

onMounted(async () => {
  await loadObjectTypes();
  await loadObjects(null);
});

function selectAll(): void {
  void loadObjects(null);
}

function selectSettings(): void {
  currentTypeId.value = "__settings__";
}

function selectType(id: string): void {
  void loadObjects(id);
}
</script>

<template>
  <DesktopChrome platform="windows">
    <template #sidebar>
      <aside class="sidebar">
        <div class="sidebar-section">
          <SidebarItem
            label="Всё"
            color="#ef4444"
            :active="!showSettings && currentTypeId === null"
            @click="selectAll"
          />
          <SidebarItem
            label="Настройки"
            color="#9ca3af"
            :active="showSettings"
            @click="selectSettings"
          />
        </div>

        <div class="sidebar-divider"></div>

        <div class="sidebar-section">
          <div class="sidebar-header">Типы</div>
          <SidebarItem
            v-for="t in objectTypes"
            :key="t.id"
            :label="t.name"
            :color="colorForType(t.id)"
            :active="!showSettings && currentTypeId === t.id"
            @click="selectType(t.id)"
          />
        </div>

      </aside>
    </template>

    <DesktopContentSurface
      :padding-top="'0'"
      :padding-inline="'0'"
      class="main-surface"
    >
      <header class="main-header">
        <h1>{{ titleLabel }}</h1>
      </header>
      <div class="main-body">
        <div v-if="showSettings" class="settings-stub">
          Настройки — в разработке.
        </div>
        <ObjectTable v-else :rows="objects" :loading="objectsLoading" />
      </div>
    </DesktopContentSurface>
  </DesktopChrome>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  width: 240px;
  height: 100%;
  background: var(--sidebar-bg, var(--background));
  padding: 16px 12px;
  gap: 8px;
  box-sizing: border-box;
}

.sidebar-section {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sidebar-divider {
  height: 1px;
  background: var(--border);
  margin: 8px 0;
}

.sidebar-header {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  padding: 4px 10px 6px;
}

.main-surface {
  border-left: 1px solid var(--border);
}

.main-header {
  padding: 16px 24px;
  border-bottom: 1px solid var(--border);
  text-align: center;
}

.main-header h1 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}

.main-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.settings-stub {
  padding: 48px;
  text-align: center;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 13px;
}
</style>
