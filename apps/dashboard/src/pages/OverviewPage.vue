<script setup lang="ts">
import { computed } from "vue";
import { Database, Eye, MoonStar, Radar } from "lucide-vue-next";
import { useDashboardData } from "@/composables/useDashboardData";
import { formatCompactHours, formatDateTimeLabel } from "@/utils/format";
import EmptyStatePanel from "@/components/dashboard/EmptyStatePanel.vue";
import MetricCard from "@/components/dashboard/MetricCard.vue";
import RangePills from "@/components/dashboard/RangePills.vue";
import SectionPanel from "@/components/dashboard/SectionPanel.vue";
import TopAppsList from "@/components/dashboard/TopAppsList.vue";
import TrendChart from "@/components/dashboard/TrendChart.vue";
import UsageHeatmap from "@/components/dashboard/UsageHeatmap.vue";

const dashboard = useDashboardData();

const summary = computed(() => dashboard.snapshot.value?.summary);
const snapshotStatus = computed(() => dashboard.snapshot.value?.status);

const sourceLabel = computed(() => {
  switch (snapshotStatus.value?.source) {
    case "selected":
      return "Выбрана вручную";
    case "explicit":
      return "Указана явно";
    case "default":
    default:
      return "По умолчанию";
  }
});

const platformLabel = computed(() => {
  switch (dashboard.platform.value) {
    case "darwin":
      return "macOS";
    case "linux":
      return "Linux";
    case "win32":
    default:
      return "Windows";
  }
});
</script>

<template>
  <div class="overview-page" data-testid="overview-page">
    <section class="overview-page__metrics">
      <MetricCard
        data-testid="metric-tracked-apps"
        label="Отслеживаемые приложения"
        accent="gold"
        :value="String(summary?.trackedAppCount ?? 0)"
        :note="snapshotStatus?.message ?? 'Все найденные приложения уже отображаются в Ark DB.'"
      />
      <MetricCard
        data-testid="metric-focus-time"
        label="В фокусе"
        accent="blue"
        :value="formatCompactHours(summary?.totalForegroundMs ?? 0)"
        :note="`Сессий: ${summary?.sessionCount ?? 0}`"
      />
      <MetricCard
        data-testid="metric-idle-time"
        label="Простой"
        accent="mint"
        :value="formatCompactHours(summary?.totalIdleMs ?? 0)"
        :note="`Событий: ${summary?.eventCount ?? 0}`"
      />
      <MetricCard
        data-testid="metric-last-recorded"
        label="Последняя запись"
        accent="rose"
        :value="formatDateTimeLabel(summary?.lastRecordedAt ?? null)"
        :note="`Первая запись: ${formatDateTimeLabel(summary?.firstRecordedAt ?? null)}`"
      />
    </section>

    <section class="overview-page__grid">
      <SectionPanel
        title="Дневной пульс активности"
        caption="Сравнение времени в фокусе и простоя за выбранный диапазон дней."
      >
        <template #header>
          <RangePills
            :current-range="dashboard.rangeDays.value"
            @select="dashboard.setRange($event)"
          />
        </template>

        <TrendChart
          v-if="dashboard.snapshot.value && dashboard.snapshot.value.dailyTrend.length > 0"
          :points="dashboard.snapshot.value.dailyTrend"
        />
        <EmptyStatePanel
          v-else
          title="История активности пока пуста"
          message="Как только `usage-tracker` накопит несколько сессий, здесь появится дневной ритм работы."
        />
      </SectionPanel>

      <SectionPanel
        title="Топ приложений"
        caption="Приложения, которые забрали больше всего времени в активном окне."
      >
        <template #header>
          <div class="overview-page__section-icon">
            <Radar :size="16" />
          </div>
        </template>

        <TopAppsList
          v-if="dashboard.snapshot.value && dashboard.snapshot.value.topApps.length > 0"
          :items="dashboard.snapshot.value.topApps"
        />
        <EmptyStatePanel
          v-else
          title="Топ пока пуст"
          message="Нужно хотя бы несколько usage-сессий, чтобы рейтинг стал осмысленным."
        />
      </SectionPanel>
    </section>

    <section class="overview-page__grid overview-page__grid--bottom">
      <SectionPanel
        title="Когда машина действительно занята"
        caption="Почасовая тепловая карта по дням недели на основе usage-сессий."
      >
        <template #header>
          <div class="overview-page__section-icon">
            <Eye :size="16" />
          </div>
        </template>

        <UsageHeatmap
          v-if="dashboard.snapshot.value && dashboard.snapshot.value.hourlyHeatmap.length > 0"
          :cells="dashboard.snapshot.value.hourlyHeatmap"
        />
        <EmptyStatePanel
          v-else
          title="Для тепловой карты пока мало данных"
          message="Карта начнёт заполняться, когда сессии покроют больше часов в течение недели."
        />
      </SectionPanel>

      <SectionPanel
        title="Статус источника"
        caption="Текущий источник Ark DB и видимость данных через Electron bridge."
      >
        <div class="overview-page__status-stack">
          <div class="overview-page__status-chip">
            <Database :size="16" />
            <span>{{ sourceLabel }}</span>
          </div>
          <div class="overview-page__status-chip">
            <MoonStar :size="16" />
            <span>{{ platformLabel }}</span>
          </div>
        </div>

        <div class="overview-page__status-copy">
          {{
            dashboard.snapshot.value?.status.message ??
            "База читается, а dashboard получает чистый срез usage-данных без ошибок."
          }}
        </div>
      </SectionPanel>
    </section>
  </div>
</template>

<style scoped>
.overview-page {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.overview-page__metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.9rem;
}

.overview-page__grid {
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(340px, 0.85fr);
  gap: 1rem;
}

.overview-page__grid--bottom {
  grid-template-columns: minmax(0, 1.2fr) minmax(320px, 0.8fr);
}

.overview-page__section-icon {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--dashboard-panel-muted);
  color: var(--dashboard-text-soft);
}

.overview-page__status-stack {
  display: flex;
  flex-wrap: wrap;
  gap: 0.6rem;
}

.overview-page__status-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.48rem;
  padding: 0.56rem 0.78rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--dashboard-panel-muted);
  color: var(--dashboard-text-soft);
}

.overview-page__status-copy {
  margin-top: 1rem;
  color: var(--dashboard-text-soft);
  line-height: 1.6;
}

@media (max-width: 1200px) {
  .overview-page__metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .overview-page__grid,
  .overview-page__grid--bottom {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (max-width: 720px) {
  .overview-page__metrics {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
