<script setup lang="ts">
import { computed, h, onMounted, onUnmounted, type Component } from "vue";
import { PhCaretLeft, PhCaretRight, PhClock, PhDatabase } from "@phosphor-icons/vue";
import { SettingsSidebar, SettingsSidebarButton } from "@kosmos/visuals";
import ObjectTable from "../dashboard/ObjectTable.vue";
import UsageTable from "../dashboard/UsageTable.vue";
import {
  currentTypeId,
  handleObjectChangeEvent,
  loadObjects,
  loadObjectTypes,
  loadUsageRows,
  objects,
  objectsLoading,
  objectTypes,
  usageLoading,
  usageRows,
} from "../dashboard/store";
import { typeVisualFor } from "../dashboard/typeVisuals";

const showUsage = computed(() => currentTypeId.value === "__usage__");

function phosphorSidebarIcon(icon: Component, weight: "duotone" | "fill"): Component {
  return {
    inheritAttrs: false,
    setup(_, { attrs }) {
      return () => h(icon, { ...attrs, size: 14, weight });
    },
  };
}

const DatabaseDuotone = phosphorSidebarIcon(PhDatabase, "duotone");
const DatabaseFill = phosphorSidebarIcon(PhDatabase, "fill");
const ClockDuotone = phosphorSidebarIcon(PhClock, "duotone");
const ClockFill = phosphorSidebarIcon(PhClock, "fill");

function sidebarIconForType(id: string, active: boolean): Component {
  return phosphorSidebarIcon(typeVisualFor(id).icon, active ? "fill" : "duotone");
}

// Подписка на live-события ARK: новый/удалённый объект → инвалидировать кэш.
type ArkSubscribeFn = (event: string, handler: (payload: unknown) => void) => () => void;
let unsubscribeObjectUpserted: (() => void) | null = null;
let unsubscribeObjectDeleted: (() => void) | null = null;

onMounted(async () => {
  await loadObjectTypes();
  await loadObjects(null);

  const subscribe = (window as unknown as { kepler?: { ark?: { subscribe?: ArkSubscribeFn } } })
    .kepler?.ark?.subscribe;

  if (subscribe) {
    unsubscribeObjectUpserted = subscribe("object_upserted", (payload) => {
      const p = payload as { type_id?: string };
      void handleObjectChangeEvent(p.type_id ?? null);
    });
    unsubscribeObjectDeleted = subscribe("object_deleted", (payload) => {
      const p = payload as { type_id?: string };
      void handleObjectChangeEvent(p.type_id ?? null);
    });
  }
});

onUnmounted(() => {
  unsubscribeObjectUpserted?.();
  unsubscribeObjectDeleted?.();
  unsubscribeObjectUpserted = null;
  unsubscribeObjectDeleted = null;
});

function selectAll(): void {
  void loadObjects(null);
}

function selectUsage(): void {
  void loadUsageRows();
}

function selectType(id: string): void {
  void loadObjects(id);
}
</script>

<template>
  <div class="dashboard" tabindex="0">
    <div class="dashboard-shell">
      <SettingsSidebar title="Таблица данных">
        <div class="dashboard-sidebar-scroll kosmos-scroll">
          <div class="dashboard-sidebar-group">
            <SettingsSidebarButton
              :icon="currentTypeId === null ? DatabaseFill : DatabaseDuotone"
              label="Все объекты"
              icon-from="var(--destructive)"
              icon-to="color-mix(in srgb, var(--destructive) 60%, var(--background))"
              :active="!showUsage && currentTypeId === null"
              @click="selectAll"
            />
            <SettingsSidebarButton
              :icon="showUsage ? ClockFill : ClockDuotone"
              label="Затреканное время"
              icon-from="var(--accent)"
              icon-to="color-mix(in srgb, var(--accent) 60%, var(--background))"
              :active="showUsage"
              @click="selectUsage"
            />
          </div>

          <div class="dashboard-sidebar-group">
            <div class="dashboard-sidebar-header">Типы</div>
            <SettingsSidebarButton
              v-for="type in objectTypes"
              :key="type.id"
              :icon="sidebarIconForType(type.id, currentTypeId === type.id)"
              :label="type.name"
              :icon-from="typeVisualFor(type.id).from"
              :icon-to="typeVisualFor(type.id).to"
              :active="!showUsage && currentTypeId === type.id"
              @click="selectType(type.id)"
            />
          </div>
        </div>
      </SettingsSidebar>

      <div class="dashboard-content">
        <header class="dashboard-titlebar">
          <div class="dashboard-titlebar__nav">
            <button type="button" class="chrome-control" disabled title="Назад" aria-label="Назад">
              <PhCaretLeft :size="14" weight="bold" />
            </button>
            <button
              type="button"
              class="chrome-control"
              disabled
              title="Вперёд"
              aria-label="Вперёд"
            >
              <PhCaretRight :size="14" weight="bold" />
            </button>
          </div>
        </header>

        <div class="dashboard-body">
          <UsageTable v-if="showUsage" :rows="usageRows" :loading="usageLoading" />
          <ObjectTable v-else :rows="objects" :loading="objectsLoading" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  width: 100%;
  height: 100%;
  flex-direction: column;
  background: var(--main-background-color);
  outline: none;
}

.dashboard-shell {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1;
}

.dashboard-sidebar-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.dashboard-sidebar-scroll {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 24px;
  margin-right: -8px;
  overflow-y: auto;
}

.dashboard-sidebar-header {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  padding: 4px 10px 6px;
}

.dashboard-content {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  background: var(--main-background-color);
}

.dashboard-titlebar {
  display: flex;
  align-items: center;
  min-height: 36px;
  padding: 8px 140px 4px 10px;
  -webkit-app-region: drag;
}

.dashboard-titlebar__nav {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  -webkit-app-region: no-drag;
}

.chrome-control {
  display: inline-flex;
  width: 28px;
  height: 28px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 72%, transparent);
  -webkit-app-region: no-drag;
}

.chrome-control:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.chrome-control:disabled {
  cursor: default;
  opacity: 0.32;
}

.dashboard-body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

@media (max-width: 760px) {
  :deep(.kosmos-settings-sidebar) {
    width: 188px;
    min-width: 188px;
  }

  :deep(.kosmos-settings-sidebar-button) {
    width: 172px;
  }

  .dashboard-titlebar {
    padding-right: 10px;
  }
}

@media (max-width: 560px) {
  :deep(.kosmos-settings-sidebar) {
    width: 56px;
    min-width: 56px;
    padding: 8px;
  }

  :deep(.kosmos-settings-sidebar__title),
  :deep(.kosmos-settings-sidebar-button__label),
  .dashboard-sidebar-header {
    display: none;
  }

  :deep(.kosmos-settings-sidebar__content) {
    gap: 16px;
  }

  :deep(.kosmos-settings-sidebar-button) {
    width: 40px;
    height: 34px;
    justify-content: center;
    padding: 6px;
  }
}
</style>
