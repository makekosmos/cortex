<script setup lang="ts">
import { computed } from "vue";
import { Tooltip } from "@kosmos/visuals";
import type { CoderActivityDay } from "./types";

defineProps<{ days: CoderActivityDay[] }>();

const dateFormatter = new Intl.DateTimeFormat("ru", { day: "numeric", month: "long" });
const monthFormatter = new Intl.DateTimeFormat("ru", { month: "short" });
const monthLabels = computed(() =>
  Array.from({ length: 12 }, (_, index) => {
    const date = new Date();
    date.setDate(1);
    date.setMonth(date.getMonth() - 11 + index);
    return monthFormatter.format(date).replace(".", "");
  }),
);
function dayLabel(day: CoderActivityDay): string {
  return `${dateFormatter.format(new Date(`${day.date}T12:00:00`))} · отправок: ${day.count}`;
}

function tooltipPlacement(index: number): "top-start" | "top" | "top-end" {
  if (index < 7) return "top-start";
  if (index >= 364) return "top-end";
  return "top";
}
</script>

<template>
  <section class="coder-panel">
    <header>
      <div>
        <h2>Активность</h2>
      </div>
      <div class="legend" aria-label="Интенсивность активности">
        <span>Меньше</span>
        <i v-for="level in [0, 1, 2, 3, 4]" :key="level" :class="`level-${level}`" />
        <span>Больше</span>
      </div>
    </header>
    <div class="activity-grid" aria-label="Календарь отправок">
      <Tooltip
        v-for="(day, index) in days"
        :key="day.date"
        class="activity-tooltip"
        :text="dayLabel(day)"
        :placement="tooltipPlacement(index)"
        :focusable="day.count > 0"
      >
        <span :class="['activity-cell', `level-${day.level}`]" aria-hidden="true" />
      </Tooltip>
    </div>
    <div class="month-labels" aria-hidden="true">
      <span v-for="month in monthLabels" :key="month">{{ month }}</span>
    </div>
  </section>
</template>

<style scoped>
.coder-panel {
  padding: 8px 0;
}

header,
.legend {
  display: flex;
  align-items: center;
}

header {
  justify-content: space-between;
  gap: 16px;
}

h2 {
  margin: 0;
}

h2 {
  color: var(--foreground);
  font-size: 0.875rem;
}

.legend {
  color: var(--muted-foreground);
  font-size: 0.6875rem;
}

.legend {
  gap: 4px;
}

.legend i,
.activity-cell {
  border-radius: 3px;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
}

.legend i {
  width: 10px;
  height: 10px;
}

.activity-grid {
  display: grid;
  aspect-ratio: 53 / 7;
  grid-auto-flow: column;
  grid-template-rows: repeat(7, 1fr);
  grid-template-columns: repeat(53, minmax(6px, 1fr));
  gap: 3px;
  margin-top: 16px;
}

.activity-tooltip {
  aspect-ratio: 1;
  min-width: 0;
}

.activity-cell {
  width: 100%;
  height: 100%;
}

.month-labels {
  display: flex;
  justify-content: space-between;
  margin-top: 7px;
  color: var(--muted-foreground);
  font-size: 0.625rem;
}

.level-1 {
  background: color-mix(in srgb, var(--coder-accent) 22%, transparent) !important;
}
.level-2 {
  background: color-mix(in srgb, var(--coder-accent) 38%, transparent) !important;
}
.level-3 {
  background: color-mix(in srgb, var(--coder-accent) 56%, transparent) !important;
}
.level-4 {
  background: color-mix(in srgb, var(--coder-accent) 76%, transparent) !important;
}
</style>
