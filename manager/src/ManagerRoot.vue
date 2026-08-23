<script setup lang="ts">
import {
  computed,
  nextTick,
  onMounted,
  ref,
  useTemplateRef,
  type Component,
  type ComponentPublicInstance,
} from "vue";
import {
  DesktopChrome,
  DesktopContentSurface,
  SettingsSidebar,
  SettingsSidebarButton,
} from "@kosmos/visuals";
import {
  Activity,
  Database,
  Package,
  RefreshCw,
  Settings2,
} from "@lucide/vue";
import { useManagerClient } from "./composables/useManagerClient";
import DataView from "./views/DataView.vue";
import SyncView from "./views/SyncView.vue";
import StoreView from "./views/StoreView.vue";
import DiagnosticsView from "./views/DiagnosticsView.vue";
import EngineSettingsView from "./views/EngineSettingsView.vue";
import SettingsView from "./views/SettingsView.vue";
import DictationSettingsView from "./views/DictationSettingsView.vue";
import FocusView from "./views/FocusView.vue";
import ConnectionsView from "./views/ConnectionsView.vue";
import AboutView from "./views/AboutView.vue";
import UpdatesView from "./views/UpdatesView.vue";

type ViewId =
  | "data"
  | "sync"
  | "packages"
  | "diagnostics"
  | "engine"
  | "settings"
  | "dictation"
  | "focus"
  | "connections"
  | "about"
  | "updates";
const client = useManagerClient();
const { error } = client;
const view = ref<ViewId>("data");
const surface = useTemplateRef<ComponentPublicInstance>("surface");
const scrollPositions = new Map<ViewId, number>();
const views: Record<ViewId, { label: string; hint: string; component: Component }> = {
  data: { label: "Данные", hint: "Типы и объекты", component: DataView },
  sync: {
    label: "Синхронизация",
    hint: "Устройства и связи",
    component: SyncView,
  },
  packages: {
    label: "Маркетплейс",
    hint: "Приложения и интеграции",
    component: StoreView,
  },
  diagnostics: {
    label: "Диагностика",
    hint: "Состояние системы",
    component: DiagnosticsView,
  },
  engine: {
    label: "Движок",
    hint: "Настройки запуска",
    component: EngineSettingsView,
  },
  settings: {
    label: "Настройки",
    hint: "Запуск Kosmos",
    component: SettingsView,
  },
  dictation: {
    label: "Диктовка и AI",
    hint: "Микрофон, модели и ключи",
    component: DictationSettingsView,
  },
  focus: { label: "Фокус", hint: "Блок-листы и служба", component: FocusView },
  connections: {
    label: "Подключения",
    hint: "Источники данных",
    component: ConnectionsView,
  },
  about: {
    label: "О приложении",
    hint: "Версия и сведения о Kosmos",
    component: AboutView,
  },
  updates: {
    label: "Обновления",
    hint: "Kosmos Desktop и приложения",
    component: UpdatesView,
  },
};
const icons = {
  data: Database,
  sync: RefreshCw,
  packages: Package,
  diagnostics: Activity,
  engine: Settings2,
  settings: Settings2,
  dictation: Settings2,
  focus: Settings2,
  connections: RefreshCw,
  about: Settings2,
  updates: RefreshCw,
};
const mainViewIds: Exclude<ViewId, "about">[] = [
  "data",
  "sync",
  "packages",
  "diagnostics",
  "engine",
  "settings",
  "dictation",
  "focus",
  "connections",
  "updates",
];
const active = computed(() => views[view.value]);

async function select(next: ViewId) {
  const element = surface.value?.$el as HTMLElement | undefined;
  if (element) scrollPositions.set(view.value, element.scrollTop);
  view.value = next;
  await nextTick();
  element?.scrollTo({ top: scrollPositions.get(next) ?? 0 });
}

onMounted(() => {
  void client.call("getHealth", undefined, "health");
});
</script>

<template>
  <DesktopChrome
    appearance="settings"
    platform="windows"
    titlebar-above-sidebar
    class="manager-chrome"
  >
    <template #titlebar-leading>
      <span class="manager-titlebar-brand">Cosmos</span>
    </template>
    <template #sidebar>
      <SettingsSidebar aria-label="Разделы менеджера" background="var(--bg-app)">
        <div class="manager-sidebar-scroll kosmos-scroll">
          <div class="manager-sidebar-group">
            <SettingsSidebarButton
              v-for="id in mainViewIds"
              :key="id"
              :icon="icons[id]"
              :label="views[id].label"
              :title="views[id].hint"
              :active="view === id"
              @click="select(id)"
            />
          </div>
        </div>
        <div class="manager-sidebar-footer">
          <SettingsSidebarButton
            :icon="icons.about"
            label="О приложении"
            title="Версия и сведения о Kosmos"
            :active="view === 'about'"
            @click="select('about')"
          />
        </div>
      </SettingsSidebar>
    </template>
    <DesktopContentSurface ref="surface" :scrollable="true" class="surface kosmos-scroll">
      <header class="page-header" :class="{ 'page-header--connections': view === 'connections' }">
        <div>
          <h1>{{ active.label }}</h1>
        </div>
      </header>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <KeepAlive>
        <component :is="active.component" :client="client" />
      </KeepAlive>
    </DesktopContentSurface>
  </DesktopChrome>
</template>
