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
  TitlebarButton,
} from "@kosmos/visuals";
import {
  PhArrowsClockwise,
  PhBrowser,
  PhDatabase,
  PhEngine,
  PhGear,
  PhInfo,
  PhKey,
  PhPackage,
  PhPlugsConnected,
  PhStorefront,
} from "@phosphor-icons/vue";
import { ChevronLeft } from "@lucide/vue";
import { useManagerClient } from "./composables/useManagerClient";
import DataView from "./views/DataView.vue";
import SyncView from "./views/SyncView.vue";
import StoreView from "./views/StoreView.vue";
import EngineSettingsView from "./views/EngineSettingsView.vue";
import SettingsView from "./views/SettingsView.vue";
import ConnectionsView from "./views/ConnectionsView.vue";
import AboutView from "./views/AboutView.vue";
import UpdatesView from "./views/UpdatesView.vue";
import SecretsView from "./views/SecretsView.vue";
import BrowserSettingsView from "./views/BrowserSettingsView.vue";

type ViewId =
  | "data"
  | "sync"
  | "packages"
  | "engine"
  | "settings"
  | "connections"
  | "about"
  | "updates"
  | "secrets"
  | "browser";
const client = useManagerClient();
const { error } = client;
const view = ref<ViewId>("data");
const surface = useTemplateRef<ComponentPublicInstance>("surface");
const activeView = useTemplateRef<{ backToCatalog?: () => void }>("activeView");
const storeDetailActive = ref(false);
const scrollPositions = new Map<ViewId, number>();
const views = {
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
  connections: {
    label: "Интеграции",
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
  secrets: {
    label: "Ключи",
    hint: "API-ключи и провайдеры",
    component: SecretsView,
  },
  browser: {
    label: "Браузер",
    hint: "Сессии и данные сайтов",
    component: BrowserSettingsView,
  },
};
const icons = {
  data: PhDatabase,
  sync: PhArrowsClockwise,
  packages: PhStorefront,
  engine: PhEngine,
  settings: PhGear,
  connections: PhPlugsConnected,
  about: PhInfo,
  updates: PhPackage,
  secrets: PhKey,
  browser: PhBrowser,
} satisfies Record<ViewId, { label: string; hint: string; component: Component }>;
const primaryViewIds: Exclude<ViewId, "about" | "packages" | "updates" | "settings">[] = [
  "data",
  "sync",
  "engine",
  "connections",
  "secrets",
  "browser",
];
const commerceViewIds: Extract<ViewId, "packages" | "updates">[] = ["packages", "updates"];
const active = computed(() => views[view.value]);

async function select(next: ViewId) {
  // SAFETY: the template ref targets the root element of the active view component.
  const element = surface.value?.$el as HTMLElement | undefined;
  if (element) scrollPositions.set(view.value, element.scrollTop);
  view.value = next;
  storeDetailActive.value = false;
  await nextTick();
  element?.scrollTo({ top: scrollPositions.get(next) ?? 0 });
}
function backFromStoreDetail() {
  activeView.value?.backToCatalog?.();
}
onMounted(() => {
  void client.call("getHealth", undefined, "health");
});
</script>

<template>
  <DesktopChrome appearance="settings" platform="windows">
    <template #titlebar-leading>
      <span class="kosmos-titlebar-brand">Kosmos</span>
    </template>
    <template #titlebar-content-leading>
      <TitlebarButton
        v-if="storeDetailActive"
        class="store-detail-titlebar-back"
        title="Назад к Marketplace"
        aria-label="Назад к Marketplace"
        @click="backFromStoreDetail"
      >
        <ChevronLeft :size="16" />
      </TitlebarButton>
    </template>
    <template #sidebar>
      <SettingsSidebar aria-label="Разделы менеджера" background="var(--bg-app)">
        <div class="manager-sidebar-scroll kosmos-scroll">
          <div class="manager-sidebar-group">
            <SettingsSidebarButton
              v-for="id in primaryViewIds"
              :key="id"
              :icon="icons[id]"
              :label="views[id].label"
              :title="views[id].hint"
              :active="view === id"
              icon-variant="plain"
              :icon-weight="view === id ? 'duotone' : 'regular'"
              @click="select(id)"
            />
          </div>
          <div class="manager-sidebar-group manager-sidebar-group--secondary">
            <SettingsSidebarButton
              v-for="id in commerceViewIds"
              :key="id"
              :icon="icons[id]"
              :label="views[id].label"
              :title="views[id].hint"
              :active="view === id"
              icon-variant="plain"
              :icon-weight="view === id ? 'duotone' : 'regular'"
              @click="select(id)"
            />
          </div>
        </div>
        <div class="manager-sidebar-footer">
          <div class="manager-sidebar-footer-actions">
            <SettingsSidebarButton
              :icon="icons.about"
              label=""
              title="О приложении"
              icon-only
              :active="view === 'about'"
              :icon-weight="view === 'about' ? 'duotone' : 'regular'"
              @click="select('about')"
            />
            <SettingsSidebarButton
              :icon="icons.settings"
              label=""
              title="Настройки"
              icon-only
              :active="view === 'settings'"
              :icon-weight="view === 'settings' ? 'duotone' : 'regular'"
              @click="select('settings')"
            />
          </div>
        </div>
      </SettingsSidebar>
    </template>
    <DesktopContentSurface ref="surface" :scrollable="true" class="surface kosmos-scroll">
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <KeepAlive>
        <component
          ref="activeView"
          :is="active.component"
          :client="client"
          @detailChange="storeDetailActive = $event"
        />
      </KeepAlive>
    </DesktopContentSurface>
  </DesktopChrome>
</template>
