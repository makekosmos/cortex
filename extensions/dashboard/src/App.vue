<script setup lang="ts">
import { computed, onMounted } from "vue";
import { Activity, Clock3, LayoutDashboard, RefreshCw } from "lucide-vue-next";
import { useDashboardData } from "@/composables/useDashboardData";
import { useActivePage, type DashboardPage } from "@/composables/useActivePage";
import OverviewPage from "@/pages/OverviewPage.vue";
import SessionsPage from "@/pages/SessionsPage.vue";

const dashboard = useDashboardData();
const { activePage, setPage } = useActivePage();

onMounted(() => {
  void dashboard.initialize();
});

const navItems: { id: DashboardPage; label: string; icon: typeof LayoutDashboard }[] = [
  { id: "overview", label: "Обзор", icon: LayoutDashboard },
  { id: "sessions", label: "Сессии", icon: Activity },
];

const statusLabel = computed(() => {
  if (dashboard.error.value) {
    return "Ошибка чтения";
  }
  if (dashboard.snapshot.value) {
    return "База подключена";
  }
  return "Загрузка...";
});

const statusTone = computed(() => {
  if (dashboard.error.value) return "danger" as const;
  if (dashboard.snapshot.value) return "success" as const;
  return "warning" as const;
});
</script>

<template>
  <div class="dashboard-shell">
    <aside class="dashboard-shell__sidebar">
      <div class="dashboard-shell__brand">
        <Clock3 :size="14" />
        <span>Dashboard</span>
      </div>
      <nav class="dashboard-shell__nav">
        <button
          v-for="item in navItems"
          :key="item.id"
          type="button"
          :class="[
            'dashboard-shell__nav-item',
            activePage === item.id ? 'dashboard-shell__nav-item--active' : '',
          ]"
          :data-testid="`nav-${item.id}`"
          @click="setPage(item.id)"
        >
          <component :is="item.icon" :size="15" />
          <span>{{ item.label }}</span>
        </button>
      </nav>
    </aside>

    <main class="dashboard-shell__main">
      <header class="dashboard-shell__topbar">
        <div class="dashboard-shell__masthead">
          <div class="dashboard-shell__eyebrow">
            <Clock3 :size="13" />
            <span>Ark observability</span>
          </div>
          <h1 class="dashboard-shell__title">Панель активности</h1>
          <p class="dashboard-shell__subtitle">
            Локальный обзор usage-сессий, приложений и данных Ark DB
            внутри Kepler shell extension'а.
          </p>
        </div>

        <div class="dashboard-shell__toolbar">
          <div
            :class="['dashboard-shell__status', `dashboard-shell__status--${statusTone}`]"
            :data-testid="'dashboard-status'"
            :title="dashboard.error.value ?? statusLabel"
          >
            <span class="dashboard-shell__status-dot" />
            <span>{{ statusLabel }}</span>
          </div>
          <button
            type="button"
            class="dashboard-shell__icon-button"
            data-testid="refresh-button"
            title="Обновить данные"
            :disabled="dashboard.isLoading.value"
            @click="dashboard.loadSnapshot()"
          >
            <RefreshCw :size="14" />
          </button>
        </div>
      </header>

      <div class="dashboard-shell__content" data-testid="dashboard-scroll-pane">
        <OverviewPage v-if="activePage === 'overview'" />
        <SessionsPage v-else-if="activePage === 'sessions'" />
      </div>
    </main>
  </div>
</template>

<style scoped>
.dashboard-shell {
  display: grid;
  grid-template-columns: 200px minmax(0, 1fr);
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.dashboard-shell__sidebar {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 0.75rem 0.75rem;
  border-right: 1px solid var(--dashboard-border-strong);
  background: var(--sidebar-background, var(--background));
  /* отступ под Electron titleBarOverlay (height: 36) */
  padding-top: 44px;
}

.dashboard-shell__brand {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0 0.6rem;
  color: var(--dashboard-text-soft);
  font-size: 0.78rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.dashboard-shell__nav {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.dashboard-shell__nav-item {
  display: inline-flex;
  align-items: center;
  gap: 0.55rem;
  padding: 0.5rem 0.65rem;
  border-radius: 10px;
  color: var(--dashboard-text-soft);
  font-size: 0.92rem;
  transition: background-color 120ms ease, color 120ms ease;
  text-align: left;
}

.dashboard-shell__nav-item:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.dashboard-shell__nav-item--active {
  background: var(--surface);
  color: var(--foreground);
  border: 1px solid var(--border);
}

.dashboard-shell__main {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  /* отступ под Electron titleBarOverlay (height: 36) */
  padding-top: 8px;
  overflow: hidden;
}

.dashboard-shell__topbar {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 1.1rem;
  margin: 8px 1rem 0;
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
  font-size: clamp(1.4rem, 2vw, 2rem);
  line-height: 1.05;
  letter-spacing: -0.03em;
  font-weight: 620;
}

.dashboard-shell__subtitle {
  max-width: 60ch;
  margin: 0.55rem 0 0;
  color: var(--dashboard-text-soft);
  font-size: 0.92rem;
  line-height: 1.55;
}

.dashboard-shell__toolbar {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  height: fit-content;
}

.dashboard-shell__status {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.4rem 0.7rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--dashboard-panel-muted);
  color: var(--dashboard-text-soft);
  font-size: 0.82rem;
}

.dashboard-shell__status-dot {
  width: 0.55rem;
  height: 0.55rem;
  border-radius: 999px;
  background: currentColor;
}

.dashboard-shell__status--success {
  color: var(--dashboard-mint);
}
.dashboard-shell__status--warning {
  color: var(--dashboard-gold);
}
.dashboard-shell__status--danger {
  color: var(--dashboard-rose);
}

.dashboard-shell__icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 10px;
  color: var(--dashboard-text-soft);
  border: 1px solid var(--border);
  background: var(--dashboard-panel-muted);
  transition: background-color 120ms ease, color 120ms ease;
}

.dashboard-shell__icon-button:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.dashboard-shell__icon-button:disabled {
  opacity: 0.6;
  cursor: default;
}

.dashboard-shell__content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 1rem;
}
</style>
