<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import {
  Activity,
  Clock3,
  Database,
  LayoutDashboard,
  PanelLeft,
  RefreshCw,
  RotateCcw,
} from "lucide-vue-next";
import {
  DesktopChrome,
  DesktopContentSurface,
  Sidebar as KeplerSidebar,
  StatusDot,
  TitlebarHistoryControls,
  type SidebarConfig,
  type SidebarNavItem,
  type StatusDotTone,
  type TitlebarPlatform,
} from "@kepler/visuals";
import { useDashboardData } from "@/composables/useDashboardData";

type RouterHistoryStateLike = {
  back?: string | null;
  forward?: string | null;
};

const STORAGE_KEY = "dashboard-sidebar-config";

function loadConfig(): Partial<SidebarConfig> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      return JSON.parse(raw) as Partial<SidebarConfig>;
    }
  } catch {
    // ignore invalid persisted config
  }

  return {};
}

function saveConfig(config: SidebarConfig) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
}

const route = useRoute();
const router = useRouter();
const dashboard = useDashboardData();

const fallbackPlatform: TitlebarPlatform = navigator.platform.startsWith("Mac")
  ? "mac"
  : navigator.platform.startsWith("Linux")
    ? "linux"
    : "windows";

const initialSidebarConfig = loadConfig();
const sidebarConfig = ref<SidebarConfig>({
  width: initialSidebarConfig.width ?? 200,
  hidden: initialSidebarConfig.hidden ?? false,
});

function persistSidebarConfig(config: SidebarConfig) {
  sidebarConfig.value = config;
  saveConfig(config);
}

function setSidebarHidden(hidden: boolean) {
  persistSidebarConfig({
    ...sidebarConfig.value,
    hidden,
  });
}

function toggleSidebar() {
  setSidebarHidden(!sidebarConfig.value.hidden);
}

const historyState = computed(() => {
  void route.fullPath;
  return (router.options.history.state as RouterHistoryStateLike | undefined) ?? undefined;
});

const canGoBack = computed(() => Boolean(historyState.value?.back));
const canGoForward = computed(() => Boolean(historyState.value?.forward));

function navigateBack() {
  if (!canGoBack.value) return;
  router.back();
}

function navigateForward() {
  if (!canGoForward.value) return;
  router.forward();
}

onMounted(() => {
  void dashboard.initialize();
});

const primaryItems = computed<SidebarNavItem[]>(() => [
  {
    id: "overview",
    icon: LayoutDashboard,
    label: "Обзор",
    active: route.name === "overview",
    onClick: () => {
      void router.push({ name: "overview" });
    },
  },
  {
    id: "sessions",
    icon: Activity,
    label: "Сессии",
    active: route.name === "sessions",
    onClick: () => {
      void router.push({ name: "sessions" });
    },
  },
]);

const statusTone = computed<StatusDotTone>(() => {
  if (dashboard.error.value) {
    return "danger";
  }
  if (dashboard.snapshot.value?.status.readable) {
    return "success";
  }
  return "warning";
});

const statusLabel = computed(() => {
  if (dashboard.error.value) {
    return "Ошибка чтения";
  }
  if (dashboard.snapshot.value?.status.readable) {
    return "База подключена";
  }
  return "База не выбрана";
});

const chromePlatform = computed<TitlebarPlatform>(() => {
  switch (dashboard.platform.value) {
    case "darwin":
      return "mac";
    case "linux":
      return "linux";
    case "win32":
    default:
      return fallbackPlatform;
  }
});

const dbPathLabel = computed(
  () => dashboard.snapshot.value?.status.path ?? "Ark DB ещё не выбрана",
);

const statusPopoverCopy = computed(() => {
  if (dashboard.error.value) {
    return dashboard.error.value;
  }

  return dashboard.snapshot.value?.status.message ?? statusLabel.value;
});
</script>

<template>
  <DesktopChrome
    class="dashboard-shell"
    :platform="chromePlatform"
  >
    <template #titlebar-leading>
      <button
        type="button"
        class="dashboard-shell__titlebar-button"
        data-testid="sidebar-toggle"
        :title="sidebarConfig.hidden ? 'Показать боковую панель' : 'Скрыть боковую панель'"
        @click="toggleSidebar()"
      >
        <PanelLeft :size="14" />
      </button>

      <TitlebarHistoryControls
        :back-disabled="!canGoBack"
        :forward-disabled="!canGoForward"
        back-title="Назад"
        forward-title="Вперёд"
        @back="navigateBack"
        @forward="navigateForward"
      />

      <button
        type="button"
        class="dashboard-shell__titlebar-button dashboard-shell__titlebar-button--label"
        data-testid="choose-database-button"
        @click="dashboard.chooseDatabase()"
      >
        <Database :size="14" />
        <span>Выбрать Ark DB</span>
      </button>

      <button
        type="button"
        class="dashboard-shell__titlebar-button"
        data-testid="refresh-button"
        title="Обновить данные"
        @click="dashboard.loadSnapshot()"
      >
        <RefreshCw :size="14" />
      </button>

      <button
        type="button"
        class="dashboard-shell__titlebar-button"
        data-testid="reset-database-button"
        title="Сбросить выбранную базу"
        @click="dashboard.resetDatabase()"
      >
        <RotateCcw :size="14" />
      </button>
    </template>

    <template #titlebar-trailing>
      <StatusDot
        class="dashboard-shell__status-dot"
        :tone="statusTone"
        :label="statusLabel"
        data-testid="dashboard-status"
      >
        <div class="dashboard-shell__status-popover">
          <p class="dashboard-shell__status-popover-title">Ark DB</p>
          <p class="dashboard-shell__status-popover-copy">{{ statusPopoverCopy }}</p>
          <p class="dashboard-shell__status-popover-path">{{ dbPathLabel }}</p>
        </div>
      </StatusDot>
    </template>

    <template #sidebar>
      <KeplerSidebar
        :primary-items="primaryItems"
        :footer-items="[]"
        :is-mac="chromePlatform === 'mac'"
        :default-width="200"
        :min-width="160"
        :max-width="320"
        :hidden-width="0"
        toggle-shortcut="meta+b|ctrl+b"
        :initial-config="sidebarConfig"
        :hidden="sidebarConfig.hidden"
        :show-toggle="false"
        :reserve-top-inset="false"
        @config-change="persistSidebarConfig"
        @update:hidden="setSidebarHidden"
      />
    </template>

    <DesktopContentSurface
      class="dashboard-shell__surface"
      data-testid="dashboard-scroll-pane"
      scrollable
      :show-left-border="!sidebarConfig.hidden"
      :radius-top-left="sidebarConfig.hidden ? '0px' : '16px'"
    >
      <div class="dashboard-shell__main">
        <header class="dashboard-shell__header">
          <div class="dashboard-shell__masthead">
            <div class="dashboard-shell__eyebrow">
              <Clock3 :size="13" />
              <span>Ark observability</span>
            </div>
            <h1 class="dashboard-shell__title">Панель активности</h1>
            <p class="dashboard-shell__subtitle">
              Локальный обзор usage-сессий, приложений и данных Ark DB без отдельной
              витринной стилизации.
            </p>
          </div>
        </header>

        <div class="dashboard-shell__path-row">
          <span class="dashboard-shell__path-label">Источник</span>
          <code class="dashboard-shell__path" data-testid="dashboard-db-path">
            {{ dbPathLabel }}
          </code>
        </div>

        <main class="dashboard-shell__content">
          <div class="dashboard-shell__content-inner">
            <RouterView />
          </div>
        </main>
      </div>
    </DesktopContentSurface>
  </DesktopChrome>
</template>

<style scoped>
.dashboard-shell__surface {
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.dashboard-shell__main {
  min-width: 0;
  padding: 1rem 1rem 0;
}

.dashboard-shell__titlebar-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  min-width: var(--kepler-titlebar-control-size, 32px);
  height: var(--kepler-titlebar-control-size, 32px);
  padding: 0 0.625rem;
  border-radius: var(--kepler-titlebar-control-radius, 10px);
  color: var(--dashboard-text-soft);
  transition:
    background-color 120ms ease,
    color 120ms ease,
    border-color 120ms ease;
}

.dashboard-shell__titlebar-button:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.dashboard-shell__titlebar-button--label {
  background: color-mix(in srgb, var(--sidebar-foreground) 8%, transparent);
  color: var(--sidebar-foreground);
}

.dashboard-shell__titlebar-button--label:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 14%, transparent);
}

.dashboard-shell__header {
  display: flex;
  gap: 0.75rem;
  padding: 1rem 1.1rem;
  border: 1px solid var(--dashboard-border-strong);
  border-radius: 16px;
  background: var(--dashboard-panel);
}

.dashboard-shell__masthead {
  max-width: 760px;
}

.dashboard-shell__eyebrow {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  margin-bottom: 0.65rem;
  color: var(--dashboard-text-muted);
  font-size: 0.78rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.dashboard-shell__title {
  margin: 0;
  color: var(--foreground);
  font-size: clamp(1.8rem, 2.4vw, 2.6rem);
  line-height: 1.05;
  letter-spacing: -0.03em;
  font-weight: 620;
}

.dashboard-shell__subtitle {
  max-width: 60ch;
  margin: 0.55rem 0 0;
  color: var(--dashboard-text-soft);
  font-size: 0.94rem;
  line-height: 1.6;
}

.dashboard-shell__status-popover {
  display: grid;
  gap: 0.35rem;
}

.dashboard-shell__status-popover-title {
  margin: 0;
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--dashboard-text-muted);
}

.dashboard-shell__status-popover-copy {
  margin: 0;
  color: var(--foreground);
  font-size: 0.84rem;
  line-height: 1.45;
}

.dashboard-shell__status-popover-path {
  margin: 0.15rem 0 0;
  color: var(--dashboard-text-soft);
  font-size: 0.75rem;
  line-height: 1.45;
  word-break: break-all;
}

.dashboard-shell__status-dot {
  transform: translateX(6px);
}

.dashboard-shell__path-row {
  display: flex;
  align-items: center;
  gap: 0.7rem;
  margin-top: 0.9rem;
  padding: 0 0.1rem;
  color: var(--dashboard-text-soft);
  font-size: 0.82rem;
}

.dashboard-shell__path-label {
  color: var(--dashboard-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.dashboard-shell__path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
  padding: 0.3rem 0.55rem;
  border-radius: 10px;
  background: var(--dashboard-panel-subtle);
  color: var(--foreground);
  border: 1px solid var(--border);
}

.dashboard-shell__content {
  display: block;
  margin-top: 0.9rem;
  padding-right: 0.15rem;
}

.dashboard-shell__content-inner {
  display: block;
}

@media (max-width: 960px) {
  .dashboard-shell__main {
    padding-inline: 0.9rem;
  }

  :deep(.kepler-status-dot-anchor) {
    display: none;
  }
}
</style>
