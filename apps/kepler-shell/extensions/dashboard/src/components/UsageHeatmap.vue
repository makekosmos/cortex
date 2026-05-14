<script setup lang="ts">
import { computed } from "vue";
import type { HourlyHeatmapCell } from "@/types/analytics";
import { formatCompactHours, weekdayLabel } from "@/utils/format";

const props = defineProps<{
  cells: HourlyHeatmapCell[];
}>();

const maxValue = computed(() => Math.max(1, ...props.cells.map((cell) => cell.foregroundMs)));

const heatmapGrid = computed(() =>
  Array.from({ length: 7 }, (_, weekday) =>
    Array.from({ length: 24 }, (_, hour) => {
      const cell = props.cells.find((entry) => entry.weekday === weekday && entry.hour === hour);
      const value = cell?.foregroundMs ?? 0;
      const intensity = value / maxValue.value;
      return {
        weekday,
        hour,
        value,
        style: {
          backgroundColor:
            value === 0
              ? "color-mix(in oklab, var(--foreground) 3%, transparent)"
              : `color-mix(in oklab, var(--foreground) ${8 + intensity * 28}%, var(--surface))`,
        },
      };
    }),
  ),
);
</script>

<template>
  <div class="usage-heatmap">
    <div class="usage-heatmap__hours">
      <span v-for="hour in [0, 4, 8, 12, 16, 20]" :key="hour">{{ hour }}:00</span>
    </div>

    <div class="usage-heatmap__rows">
      <div v-for="row in heatmapGrid" :key="row[0].weekday" class="usage-heatmap__row">
        <div class="usage-heatmap__weekday">{{ weekdayLabel(row[0].weekday) }}</div>
        <div class="usage-heatmap__cells">
          <div
            v-for="cell in row"
            :key="`${cell.weekday}-${cell.hour}`"
            class="usage-heatmap__cell"
            :style="cell.style"
            :title="`${weekdayLabel(cell.weekday)} ${cell.hour}:00 — ${formatCompactHours(cell.value)}`"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.usage-heatmap {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.usage-heatmap__hours {
  display: grid;
  grid-template-columns: 56px repeat(6, 1fr);
  color: var(--dashboard-text-muted);
  font-size: 0.75rem;
}

.usage-heatmap__hours span:first-child {
  grid-column: 2;
}

.usage-heatmap__rows {
  display: flex;
  flex-direction: column;
  gap: 0.32rem;
}

.usage-heatmap__row {
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr);
  gap: 0.55rem;
  align-items: center;
}

.usage-heatmap__weekday {
  color: var(--dashboard-text-soft);
  font-size: 0.82rem;
}

.usage-heatmap__cells {
  display: grid;
  grid-template-columns: repeat(24, minmax(0, 1fr));
  gap: 0.2rem;
}

.usage-heatmap__cell {
  aspect-ratio: 1 / 1;
  border-radius: 4px;
  border: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
}
</style>
