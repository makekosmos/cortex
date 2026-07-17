<script setup lang="ts">
import { computed } from "vue";
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
      <span
        v-for="day in days"
        :key="day.date"
        :class="`level-${day.level}`"
        :data-tooltip="dayLabel(day)"
        :aria-label="dayLabel(day)"
        :tabindex="day.count > 0 ? 0 : -1"
      />
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
.activity-grid span {
  border-radius: 3px;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
}

.legend i {
  width: 10px;
  height: 10px;
}

.activity-grid {
  display: grid;
  grid-auto-flow: column;
  grid-template-rows: repeat(7, 1fr);
  grid-template-columns: repeat(53, minmax(6px, 1fr));
  gap: 3px;
  margin-top: 16px;
}

.activity-grid span {
  position: relative;
  aspect-ratio: 1;
}

.activity-grid span::after {
  position: absolute;
  z-index: 2;
  bottom: calc(100% + 7px);
  left: 50%;
  width: max-content;
  max-width: 180px;
  padding: 5px 7px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--popover);
  color: var(--popover-foreground);
  box-shadow: var(--shadow-floating);
  content: attr(data-tooltip);
  font-size: 0.6875rem;
  line-height: 1.3;
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, 2px);
  transition:
    opacity 100ms ease,
    transform 100ms ease;
  white-space: nowrap;
}

.activity-grid span:hover::after,
.activity-grid span:focus-visible::after {
  opacity: 1;
  transform: translate(-50%, 0);
}

.activity-grid span:nth-child(-n + 7)::after {
  left: 0;
  transform: translate(0, 2px);
}

.activity-grid span:nth-child(-n + 7):hover::after,
.activity-grid span:nth-child(-n + 7):focus-visible::after {
  transform: translate(0, 0);
}

.activity-grid span:nth-last-child(-n + 7)::after {
  right: 0;
  left: auto;
  transform: translate(0, 2px);
}

.activity-grid span:nth-last-child(-n + 7):hover::after,
.activity-grid span:nth-last-child(-n + 7):focus-visible::after {
  transform: translate(0, 0);
}

.month-labels {
  display: flex;
  justify-content: space-between;
  margin-top: 7px;
  color: var(--muted-foreground);
  font-size: 0.625rem;
}

.level-1 {
  background: color-mix(in srgb, var(--accent) 28%, transparent) !important;
}
.level-2 {
  background: color-mix(in srgb, var(--accent) 48%, transparent) !important;
}
.level-3 {
  background: color-mix(in srgb, var(--accent) 70%, transparent) !important;
}
.level-4 {
  background: var(--accent) !important;
}
</style>
