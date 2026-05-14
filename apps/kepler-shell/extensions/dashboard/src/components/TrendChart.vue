<script setup lang="ts">
import { computed } from "vue";
import type { DailyTrendPoint } from "@/types/analytics";
import { formatCompactHours, formatDateLabel } from "@/utils/format";

const props = defineProps<{
  points: DailyTrendPoint[];
}>();

const chartWidth = 760;
const chartHeight = 220;

const maxValue = computed(() =>
  Math.max(1, ...props.points.map((point) => Math.max(point.foregroundMs, point.idleMs))),
);

const foregroundPath = computed(() =>
  props.points
    .map((point, index) => {
      const x = (index / Math.max(1, props.points.length - 1)) * chartWidth;
      const y = chartHeight - (point.foregroundMs / maxValue.value) * chartHeight;
      return `${index === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" "),
);

const idlePath = computed(() =>
  props.points
    .map((point, index) => {
      const x = (index / Math.max(1, props.points.length - 1)) * chartWidth;
      const y = chartHeight - (point.idleMs / maxValue.value) * chartHeight;
      return `${index === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" "),
);

const gridLabels = computed(() =>
  [1, 0.66, 0.33].map((ratio) => formatCompactHours(maxValue.value * ratio)),
);
</script>

<template>
  <div class="trend-chart">
    <div class="trend-chart__legend">
      <span class="trend-chart__legend-item">
        <span class="trend-chart__legend-dot trend-chart__legend-dot--foreground" />
        В фокусе
      </span>
      <span class="trend-chart__legend-item">
        <span class="trend-chart__legend-dot trend-chart__legend-dot--idle" />
        Простой
      </span>
    </div>

    <div class="trend-chart__frame">
      <div class="trend-chart__y-axis">
        <span v-for="label in gridLabels" :key="label">{{ label }}</span>
      </div>
      <svg
        class="trend-chart__svg"
        :viewBox="`0 0 ${chartWidth} ${chartHeight}`"
        preserveAspectRatio="none"
      >
        <line
          v-for="line in [0.25, 0.5, 0.75]"
          :key="line"
          x1="0"
          :y1="chartHeight * line"
          :x2="chartWidth"
          :y2="chartHeight * line"
          class="trend-chart__grid"
        />

        <path :d="foregroundPath" class="trend-chart__foreground-line" />
        <path :d="idlePath" class="trend-chart__idle-line" />
      </svg>
    </div>

    <div class="trend-chart__x-axis">
      <span v-for="point in points.slice(-6)" :key="point.date">{{ formatDateLabel(point.date) }}</span>
    </div>
  </div>
</template>

<style scoped>
.trend-chart {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.trend-chart__legend {
  display: flex;
  gap: 1rem;
  color: var(--dashboard-text-soft);
  font-size: 0.85rem;
}

.trend-chart__legend-item {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
}

.trend-chart__legend-dot {
  width: 0.65rem;
  height: 0.65rem;
  border-radius: 999px;
}

.trend-chart__legend-dot--foreground {
  background: var(--dashboard-accent);
}

.trend-chart__legend-dot--idle {
  background: var(--dashboard-blue);
}

.trend-chart__frame {
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr);
  gap: 0.8rem;
  align-items: stretch;
}

.trend-chart__y-axis {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  color: var(--dashboard-text-muted);
  font-size: 0.76rem;
}

.trend-chart__svg {
  width: 100%;
  height: 220px;
}

.trend-chart__grid {
  stroke: color-mix(in oklab, var(--border) 56%, transparent);
  stroke-width: 1;
}

.trend-chart__foreground-line {
  fill: none;
  stroke: var(--foreground);
  stroke-width: 2.5;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.trend-chart__idle-line {
  fill: none;
  stroke: var(--muted-foreground);
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
  opacity: 0.9;
}

.trend-chart__x-axis {
  display: flex;
  justify-content: space-between;
  margin-left: 56px;
  color: var(--dashboard-text-muted);
  font-size: 0.76rem;
}
</style>
