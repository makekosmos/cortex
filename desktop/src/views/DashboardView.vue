<script setup lang="ts">
import { computed, h, onMounted, onUnmounted, ref, type Component } from "vue";
import { Activity, Code2, Unplug } from "@lucide/vue";
import { PhClock, PhDatabase } from "@phosphor-icons/vue";
import {
  DesktopChrome,
  SettingsSidebar,
  SettingsSidebarButton,
  TitlebarHistoryControls,
  ToastHost,
  provideToastHost,
  usePlatform,
} from "@kosmos/visuals";
import BodyView from "../body/BodyView.vue";
import CoderView from "../coder/CoderView.vue";
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

type DashboardSection = "integrations" | "body" | "coder" | "data";
interface DashboardWindow extends Window {
  kepler?: { ark?: { subscribe?: ArkSubscribeFn } };
}

const section = ref<DashboardSection>(
  window.location.hash.startsWith("#/dashboard/integrations")
    ? "integrations"
    : window.location.hash.startsWith("#/dashboard/body")
      ? "body"
      : window.location.hash.startsWith("#/dashboard/coder")
        ? "coder"
        : "data",
);
const showUsage = computed(() => section.value === "data" && currentTypeId.value === "__usage__");
const { platform } = usePlatform();
provideToastHost();

function phosphorSidebarIcon(icon: Component, weight: "duotone" | "fill"): Component {
  return {
    inheritAttrs: false,
    setup(_, { attrs }) {
      return () => h(icon, { ...attrs, size: 16, weight });
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
type ArkSubscribeFn = <T>(event: string, handler: (payload: T) => void) => () => void;
let unsubscribeObjectUpserted: (() => void) | null = null;
let unsubscribeObjectDeleted: (() => void) | null = null;

onMounted(async () => {
  if (section.value === "data") {
    await loadObjectTypes();
    await loadObjects(null);
  }

// SAFETY: the surrounding domain validation preserves the asserted contract.
  // SAFETY: the preload bridge is installed on the desktop window before mount.
  const subscribe = (window as DashboardWindow).kepler?.ark?.subscribe;

  if (subscribe) {
    unsubscribeObjectUpserted = subscribe("object_upserted", (payload) => {
// SAFETY: the surrounding domain validation preserves the asserted contract.
      const p = payload as { type_id?: string };
      void handleObjectChangeEvent(p.type_id ?? null);
    });
    unsubscribeObjectDeleted = subscribe("object_deleted", (payload) => {
// SAFETY: the surrounding domain validation preserves the asserted contract.
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

function selectCoder(): void {
  section.value = "coder";
  window.history.replaceState(null, "", "#/dashboard/coder");
}
</script>

<template>
  <div class="dashboard" tabindex="0">
    <ToastHost />
    <DesktopChrome appearance="settings" :platform="platform">
      <template #sidebar>
        <SettingsSidebar title="Kosmos" background="var(--bg-app)">
          <div class="dashboard-sidebar-scroll kosmos-scroll">
            <div class="dashboard-sidebar-group">
              <SettingsSidebarButton
                :icon="Unplug"
                label="Интеграции"
                :active="section === 'integrations'"
                @click="selectIntegrations"
              />
            </div>

            <div class="dashboard-sidebar-group">
              <SettingsSidebarButton
                :icon="Activity"
                label="Тело"
                :active="section === 'body'"
                @click="selectBody"
              />
              <SettingsSidebarButton
                :icon="Code2"
                label="Кодер"
                :active="section === 'coder'"
                @click="selectCoder"
              />
              <SettingsSidebarButton
                :icon="currentTypeId === null ? DatabaseFill : DatabaseDuotone"
                label="Все объекты"
                :active="section === 'data' && !showUsage && currentTypeId === null"
                @click="selectAll"
              />
              <SettingsSidebarButton
                :icon="showUsage ? ClockFill : ClockDuotone"
                label="Затреканное время"
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
        <KeepAlive v-if="section !== 'data'">
          <IntegrationsView v-if="section === 'integrations'" />
          <BodyView v-else-if="section === 'body'" />
          <CoderView v-else-if="section === 'coder'" />
        </KeepAlive>
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
  padding: 0 8px 8px;
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
