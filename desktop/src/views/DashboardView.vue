<script setup lang="ts">
import { computed, h, onMounted, onUnmounted, ref, type Component } from "vue";
import { Activity, Unplug } from "@lucide/vue";
import { PhClock, PhDatabase } from "@phosphor-icons/vue";
import {
  DesktopChrome,
  SettingsSidebar,
  SidebarButton,
  TitlebarHistoryControls,
  ToastHost,
  provideToastHost,
} from "@kosmos/visuals";
import BodyView from "../body/BodyView.vue";
import ObjectTable from "../dashboard/ObjectTable.vue";
import UsageTable from "../dashboard/UsageTable.vue";
import IntegrationsView from "../integrations/IntegrationsView.vue";
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

type DashboardSection = "integrations" | "body" | "data";

const section = ref<DashboardSection>(
  window.location.hash.startsWith("#/dashboard/integrations")
    ? "integrations"
    : window.location.hash.startsWith("#/dashboard/body")
      ? "body"
      : "data",
);
const showUsage = computed(() => section.value === "data" && currentTypeId.value === "__usage__");
provideToastHost();

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
  if (section.value === "data") {
    await loadObjectTypes();
    await loadObjects(null);
  }

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
  section.value = "data";
  window.history.replaceState(null, "", "#/dashboard");
  void loadObjects(null);
}

function selectUsage(): void {
  section.value = "data";
  window.history.replaceState(null, "", "#/dashboard");
  void loadUsageRows();
}

function selectType(id: string): void {
  section.value = "data";
  window.history.replaceState(null, "", "#/dashboard");
  void loadObjects(id);
}

function selectBody(): void {
  section.value = "body";
  window.history.replaceState(null, "", "#/dashboard/body");
}

function selectIntegrations(): void {
  section.value = "integrations";
  window.history.replaceState(null, "", "#/dashboard/integrations");
}
</script>

<template>
  <div class="dashboard" tabindex="0">
    <ToastHost />
    <DesktopChrome appearance="settings" platform="windows">
      <template #sidebar>
        <SettingsSidebar title="Kosmos" background="var(--bg-app)">
          <div class="dashboard-sidebar-scroll kosmos-scroll">
            <div class="dashboard-sidebar-group">
              <SidebarButton
                :icon="Unplug"
                label="Интеграции"
                :active="section === 'integrations'"
                @click="selectIntegrations"
              />
            </div>

            <div class="dashboard-sidebar-group">
              <SidebarButton
                :icon="Activity"
                label="Тело"
                :active="section === 'body'"
                @click="selectBody"
              />
              <SidebarButton
                :icon="currentTypeId === null ? DatabaseFill : DatabaseDuotone"
                label="Все объекты"
                :active="section === 'data' && !showUsage && currentTypeId === null"
                @click="selectAll"
              />
              <SidebarButton
                :icon="showUsage ? ClockFill : ClockDuotone"
                label="Затреканное время"
                :active="showUsage"
                @click="selectUsage"
              />
            </div>

            <div class="dashboard-sidebar-group">
              <div class="dashboard-sidebar-header">Типы</div>
              <SidebarButton
                v-for="type in objectTypes"
                :key="type.id"
                :icon="sidebarIconForType(type.id, currentTypeId === type.id)"
                :label="type.name"
                :active="section === 'data' && !showUsage && currentTypeId === type.id"
                @click="selectType(type.id)"
              />
            </div>
          </div>
        </SettingsSidebar>
      </template>

      <template #titlebar-leading>
        <TitlebarHistoryControls :back-disabled="true" :forward-disabled="true" />
      </template>

      <div class="dashboard-body">
        <IntegrationsView v-if="section === 'integrations'" />
        <BodyView v-else-if="section === 'body'" />
        <UsageTable v-else-if="showUsage" :rows="usageRows" :loading="usageLoading" />
        <ObjectTable v-else :rows="objects" :loading="objectsLoading" />
      </div>
    </DesktopChrome>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  width: 100%;
  height: 100%;
  flex-direction: column;
  background: var(--bg-app);
  outline: none;
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
  overflow-x: hidden;
  overflow-y: auto;
}

.dashboard-sidebar-header {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: var(--muted-foreground);
  padding: 4px 10px 6px;
}

.dashboard-body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

@media (max-width: 560px) {
  :deep(.kosmos-settings-sidebar) {
    display: none;
  }
}
</style>
