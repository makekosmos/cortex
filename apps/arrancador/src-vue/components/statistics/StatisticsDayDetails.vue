<script setup lang="ts">
import { formatGameName } from "@vue-app/lib/statistics";
import { Clock3, Gamepad2 } from "lucide-vue-next";
import { computed } from "vue";

const props = defineProps<{
  selectedDateLabel: string;
  totalLabel: string;
  durationLabel: string;
  loading: boolean;
  error: string | null;
  rows: Array<{
    id: string;
    name: string;
    seconds: number;
    hours: number;
  }>;
}>();

const topRow = computed(() => props.rows[0] ?? null);
</script>

<template>
  <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
    <div class="flex flex-wrap items-start justify-between gap-4">
      <div class="space-y-1">
        <div class="text-sm font-[510] text-foreground">Статистика по дню</div>
        <p class="text-sm text-muted-foreground">{{ selectedDateLabel }}</p>
      </div>

      <div class="rounded-full border border-border/70 bg-background/50 px-4 py-2 text-right">
        <div class="text-xs uppercase tracking-[0.16em] text-muted-foreground">Наиграно</div>
        <div class="mt-1 text-lg font-semibold text-foreground">{{ totalLabel }}</div>
        <div class="text-xs text-muted-foreground">{{ durationLabel }}</div>
      </div>
    </div>

    <div v-if="error" class="mt-4 rounded-2xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
      {{ error }}
    </div>

    <div v-else-if="loading" class="mt-4 flex h-[220px] items-center justify-center text-sm text-muted-foreground">
      Загружаем выбранный день…
    </div>

    <div v-else class="mt-4 grid gap-4 lg:grid-cols-[220px_minmax(0,1fr)]">
      <div class="rounded-2xl border border-border/60 bg-background/35 p-4">
        <div class="flex items-center gap-2 text-sm font-medium text-foreground">
          <Clock3 class="h-4 w-4 text-muted-foreground" />
          Сводка
        </div>

        <div class="mt-4 space-y-3 text-sm">
          <div class="flex items-center justify-between gap-3">
            <span class="text-muted-foreground">Часов</span>
            <span class="font-medium text-foreground">{{ totalLabel }}</span>
          </div>
          <div class="flex items-center justify-between gap-3">
            <span class="text-muted-foreground">Игр</span>
            <span class="font-medium text-foreground">{{ rows.length }}</span>
          </div>
          <div class="space-y-1 rounded-xl border border-border/60 bg-card/60 p-3">
            <div class="text-xs uppercase tracking-[0.16em] text-muted-foreground">Топ игра</div>
            <div class="text-sm font-medium text-foreground">
              {{ topRow ? formatGameName(topRow.name) : "Без запусков" }}
            </div>
          </div>
        </div>
      </div>

      <div class="rounded-2xl border border-border/60 bg-background/35 p-4">
        <div class="flex items-center gap-2 text-sm font-medium text-foreground">
          <Gamepad2 class="h-4 w-4 text-muted-foreground" />
          Разбивка по играм
        </div>

        <div v-if="rows.length > 0" class="mt-4 space-y-3">
          <div
            v-for="row in rows"
            :key="row.id"
            class="flex items-center justify-between gap-4 rounded-xl border border-border/60 bg-card/60 px-3 py-2.5"
          >
            <div class="min-w-0">
              <div class="truncate text-sm font-medium text-foreground">{{ row.name }}</div>
              <div class="text-xs text-muted-foreground">{{ row.hours.toFixed(1) }} ч</div>
            </div>
            <div class="shrink-0 text-sm text-muted-foreground">{{ Math.round(row.seconds / 60) }} мин</div>
          </div>
        </div>

        <div v-else class="mt-4 flex h-[140px] items-center justify-center text-sm text-muted-foreground">
          В этот день запусков не было
        </div>
      </div>
    </div>
  </section>
</template>
