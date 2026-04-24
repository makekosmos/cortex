<script setup lang="ts">
import {
  formatDateWithWeekday,
  formatHours,
  type HeatmapCell,
  type HeatmapWeek,
} from "@vue-app/lib/statistics";
import { CalendarDays } from "lucide-vue-next";
import { computed } from "vue";

const props = defineProps<{
  rangeLabel: string;
  totalLabel: string;
  weeks: HeatmapWeek[];
  selectedDate: string;
  loading: boolean;
}>();

const emit = defineEmits<{
  selectDate: [date: string];
}>();

const populatedCells = computed(() =>
  props.weeks.flatMap((week) => week.cells).filter((cell) => cell.inRange && cell.seconds > 0).length,
);

function buildCellTitle(cell: HeatmapCell) {
  if (!cell.inRange) {
    return "";
  }

  return `${formatDateWithWeekday(cell.date)}: ${formatHours(cell.seconds)}`;
}

function resolveCellStyle(cell: HeatmapCell) {
  if (!cell.inRange) {
    return {
      backgroundColor: "transparent",
      borderColor: "transparent",
      opacity: "0",
    };
  }

  const accentStrength = [0, 18, 34, 52, 72][cell.level];
  return {
    backgroundColor:
      cell.level === 0
        ? "color-mix(in srgb, var(--card) 88%, var(--muted) 12%)"
        : `color-mix(in srgb, var(--accent) ${accentStrength}%, var(--card))`,
    borderColor: cell.isSelected
      ? "var(--accent)"
      : cell.isToday
        ? "color-mix(in srgb, var(--accent) 65%, white)"
        : "color-mix(in srgb, var(--border) 78%, transparent)",
    boxShadow: cell.isSelected
      ? "0 0 0 1px color-mix(in srgb, var(--accent) 65%, transparent)"
      : "none",
  };
}
</script>

<template>
  <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
    <div class="flex flex-wrap items-start justify-between gap-4">
      <div class="space-y-1">
        <div class="flex items-center gap-2 text-sm font-[510] text-foreground">
          <CalendarDays class="h-4 w-4 text-muted-foreground" />
          Активность по дням
        </div>
        <p class="text-sm text-muted-foreground">{{ rangeLabel }}</p>
      </div>

      <div class="text-right">
        <div class="text-xs uppercase tracking-[0.18em] text-muted-foreground">Всего</div>
        <div class="mt-1 text-lg font-semibold text-foreground">{{ totalLabel }}</div>
        <div class="text-xs text-muted-foreground">{{ populatedCells }} активных дней</div>
      </div>
    </div>

    <div
      class="mt-5 overflow-x-auto rounded-2xl border border-border/60 bg-background/35 p-4"
      role="img"
      aria-label="Тепловая карта игрового времени по дням"
    >
      <div v-if="loading && weeks.length === 0" class="flex h-[180px] items-center justify-center text-sm text-muted-foreground">
        Загружаем тепловую карту…
      </div>

      <div v-else class="flex min-w-max gap-3">
        <div class="grid grid-rows-[20px_repeat(7,14px)] gap-y-1 text-[11px] text-muted-foreground">
          <div />
          <span>Пн</span>
          <span />
          <span>Ср</span>
          <span />
          <span>Пт</span>
          <span />
          <span>Вс</span>
        </div>

        <div class="flex gap-1.5">
          <div v-for="week in weeks" :key="week.key" class="grid grid-rows-[20px_repeat(7,14px)] gap-y-1">
            <div class="h-5 text-[11px] text-muted-foreground">
              {{ week.monthLabel ?? "" }}
            </div>

            <button
              v-for="cell in week.cells"
              :key="cell.date"
              type="button"
              class="h-[14px] w-[14px] rounded-[4px] border transition-transform duration-150 hover:scale-[1.08] disabled:pointer-events-none"
              :class="cell.isSelected && 'ring-1 ring-accent/80'"
              :style="resolveCellStyle(cell)"
              :disabled="!cell.inRange"
              :data-date="cell.date"
              :aria-pressed="cell.isSelected"
              :aria-label="buildCellTitle(cell)"
              :title="buildCellTitle(cell)"
              @click="emit('selectDate', cell.date)"
            />
          </div>
        </div>
      </div>
    </div>
  </section>
</template>
