<script setup lang="ts">
import { BarChart3, CalendarRange } from "lucide-vue-next";
import {
  formatDateLong,
  formatDateMonthLabel,
  type StatisticsRangePreset,
  type StatisticsRangePresetOption,
} from "@vue-app/lib/statistics";

defineProps<{
  rangePreset: StatisticsRangePreset;
  rangeLabel: string;
  selectedMonthValue: string;
  startDate: string;
  endDate: string;
  monthOptions: string[];
  dayOptions: string[];
  presets: StatisticsRangePresetOption[];
}>();

const emit = defineEmits<{
  selectPreset: [presetId: StatisticsRangePreset];
  selectMonth: [value: string];
  updateStartDate: [value: string];
  updateEndDate: [value: string];
}>();
</script>

<template>
  <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
    <div class="flex items-start justify-between gap-3">
      <div>
        <div class="flex items-center gap-2 text-sm font-[510]">
          <BarChart3 class="h-4 w-4 text-muted-foreground" />
          Период
        </div>
        <p class="mt-1 text-sm text-muted-foreground">
          {{ rangeLabel || "Выберите диапазон для отчета" }}
        </p>
      </div>
      <div class="rounded-full border border-border/70 bg-background/50 p-2 text-muted-foreground">
        <CalendarRange class="h-4 w-4" />
      </div>
    </div>

    <div class="mt-4 flex flex-wrap gap-2">
      <button
        v-for="preset in presets"
        :key="preset.id"
        type="button"
        class="inline-flex h-10 items-center rounded-full border px-4 text-sm transition-colors"
        :class="
          rangePreset === preset.id
            ? 'border-primary bg-primary text-primary-foreground'
            : 'border-border/70 bg-background/40 hover:bg-accent/70'
        "
        @click="emit('selectPreset', preset.id)"
      >
        {{ preset.label }}
      </button>
    </div>

    <div class="mt-4 grid gap-3 lg:grid-cols-3">
      <label class="space-y-1">
        <span class="text-xs font-medium text-foreground/90">Месяц</span>
        <select
          id="stats-month"
          class="flex h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
          :value="selectedMonthValue"
          aria-label="Выберите месяц"
          @change="emit('selectMonth', ($event.target as HTMLSelectElement).value)"
        >
          <option value="">Выберите месяц</option>
          <option v-for="value in monthOptions" :key="value" :value="value">
            {{ formatDateMonthLabel(value) }}
          </option>
        </select>
      </label>

      <label class="space-y-1">
        <span class="text-xs font-medium text-foreground/90">С</span>
        <select
          id="stats-start-date"
          class="flex h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
          :value="startDate"
          aria-label="Выберите начальную дату"
          @change="emit('updateStartDate', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="value in dayOptions" :key="`start-${value}`" :value="value">
            {{ formatDateLong(value) }}
          </option>
        </select>
      </label>

      <label class="space-y-1">
        <span class="text-xs font-medium text-foreground/90">По</span>
        <select
          id="stats-end-date"
          class="flex h-11 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
          :value="endDate"
          aria-label="Выберите конечную дату"
          @change="emit('updateEndDate', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="value in dayOptions" :key="`end-${value}`" :value="value">
            {{ formatDateLong(value) }}
          </option>
        </select>
      </label>
    </div>
  </section>
</template>
