<script setup lang="ts">
import type { CalendarViewMode } from "@/services/calendar/contracts";

const viewMode = defineModel<CalendarViewMode>({
  required: true,
});

const options: Array<{ id: CalendarViewMode; label: string }> = [
  { id: "day", label: "День" },
  { id: "four-days", label: "4 дня" },
  { id: "week", label: "Неделя" },
  { id: "month", label: "Месяц" },
];
</script>

<template>
  <div
    class="inline-flex rounded-xl border border-(--border) bg-(--secondary)/70 p-1"
    role="tablist"
    aria-label="Вид календаря"
  >
    <button
      v-for="option in options"
      :key="option.id"
      type="button"
      class="rounded-lg px-3 py-1.5 text-sm font-medium transition-colors"
      :class="
        viewMode === option.id
          ? 'bg-(--background) text-(--foreground) shadow-sm'
          : 'text-(--muted-foreground) hover:text-(--foreground)'
      "
      role="tab"
      :aria-selected="viewMode === option.id"
      @click="viewMode = option.id"
    >
      {{ option.label }}
    </button>
  </div>
</template>
