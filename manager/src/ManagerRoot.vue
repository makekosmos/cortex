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
  PhCrosshair,
  PhBrowser,
  PhDatabase,
  PhEngine,
  PhGear,
  PhGauge,
  PhInfo,
  PhKey,
  PhMicrophone,
  PhPackage,
  PhPlugsConnected,
  PhStorefront,
} from "@phosphor-icons/vue";
import { ChevronLeft } from "@lucide/vue";
import { useManagerClient } from "./composables/useManagerClient";
import DataView from "./views/DataView.vue";
import SyncView from "./views/SyncView.vue";
import StoreView from "./views/StoreView.vue";
import DiagnosticsView from "./views/DiagnosticsView.vue";
import EngineSettingsView from "./views/EngineSettingsView.vue";
import SettingsView from "./views/SettingsView.vue";
import ConnectionsView from "./views/ConnectionsView.vue";
import AboutView from "./views/AboutView.vue";
import UpdatesView from "./views/UpdatesView.vue";
import SecretsView from "./views/SecretsView.vue";
import BrowserSettingsView from "./views/BrowserSettingsView.vue";
import type { PackageSnapshot } from "./manager-api";

type ViewId =
  | "data"
  | "sync"
  | "diagnostics"
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
  diagnostics: {
    label: "Диагностика",
    hint: "Состояние системы",
    component: DiagnosticsView,
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
  diagnostics: PhGauge,
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
  "diagnostics",
  "engine",
  "connections",
  "secrets",
  "browser",
];
const commerceViewIds: Extract<ViewId, "packages" | "updates">[] = ["packages", "updates"];
const standaloneApps = [
  ["com.kosmos.dictation", "Диктовка и AI", PhMicrophone],
  ["com.kosmos.focus", "Фокус", PhCrosshair],
] as const;
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
async function openStandaloneApp(packageId: string) {
  const packages = await client.call<PackageSnapshot>(
    "getPackages",
    { kind: "app" },
    `app-list:${packageId}`,
  );
  const installed = packages?.packages.find((item) => item.id === packageId);
  if (!installed) return select("packages");
  if (
    !installed.enabled &&
    !(await client.call(
      "setPackageEnabled",
      { package_id: installed.id, version: installed.version, enabled: true },
      `app-enable:${packageId}`,
    ))
  )
    return;
  await client.call("openPackage", { package_id: packageId }, `app-open:${packageId}`);
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
          <div class="manager-sidebar-group manager-sidebar-group--apps">
            <SettingsSidebarButton
              v-for="app in standaloneApps"
              :key="app[0]"
              :icon="app[2]"
              :label="app[1]"
              :title="app[1]"
              icon-variant="plain"
              @click="openStandaloneApp(app[0])"
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
