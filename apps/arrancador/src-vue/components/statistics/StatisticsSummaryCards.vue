<script setup lang="ts">
import { Activity, Clock3, Gamepad2 } from "lucide-vue-next";

defineProps<{
  totalLabel: string;
  rangeLabel: string;
  averageLabel: string;
  activeDaysLabel: string;
  gamesCount: number;
  topGameLabel: string;
}>();
</script>

<template>
  <div class="grid gap-4 md:grid-cols-3">
    <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
      <div class="flex items-center justify-between gap-3">
        <div class="text-sm font-medium text-muted-foreground">Всего за период</div>
        <Clock3 class="h-4 w-4 text-muted-foreground" />
      </div>
      <div class="mt-3 text-2xl font-semibold text-foreground">{{ totalLabel }}</div>
      <p class="mt-1 text-xs text-muted-foreground">{{ rangeLabel || "За последний период" }}</p>
    </section>

    <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
      <div class="flex items-center justify-between gap-3">
        <div class="text-sm font-medium text-muted-foreground">Среднее в день</div>
        <Activity class="h-4 w-4 text-muted-foreground" />
      </div>
      <div class="mt-3 text-2xl font-semibold text-foreground">{{ averageLabel }}</div>
      <p class="mt-1 text-xs text-muted-foreground">{{ activeDaysLabel }}</p>
    </section>

    <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
      <div class="flex items-center justify-between gap-3">
        <div class="text-sm font-medium text-muted-foreground">Игры за период</div>
        <Gamepad2 class="h-4 w-4 text-muted-foreground" />
      </div>
      <div class="mt-3 text-2xl font-semibold text-foreground">{{ gamesCount }}</div>
      <p class="mt-1 text-xs text-muted-foreground">{{ topGameLabel }}</p>
    </section>
  </div>
</template>
