<script setup lang="ts">
import { computed } from "vue";
import { Gamepad2 } from "lucide-vue-next";
import { formatDuration, formatGameName, type PerGamePoint } from "@vue-app/lib/statistics";

const props = defineProps<{
  data: PerGamePoint[];
  hasData: boolean;
  description: string;
  ariaLabel: string;
}>();

const maxSeconds = computed(() => Math.max(...props.data.map((entry) => entry.seconds), 1));

const rows = computed(() =>
  props.data.map((entry) => ({
    ...entry,
    width: `${Math.max(10, (entry.seconds / maxSeconds.value) * 100)}%`,
    label: formatGameName(entry.name),
    duration: formatDuration(entry.seconds),
  })),
);
</script>

<template>
  <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
    <div class="flex items-start justify-between gap-3">
      <div>
        <div class="flex items-center gap-2 text-sm font-[510]">
          <Gamepad2 class="h-4 w-4 text-muted-foreground" />
          Разбивка по играм
        </div>
        <p class="mt-1 text-sm text-muted-foreground">{{ description }}</p>
      </div>
      <div class="text-xs text-muted-foreground">{{ data.length }} игр</div>
    </div>

    <div
      class="mt-4 h-[260px] overflow-auto rounded-2xl border border-border/60 bg-background/35 p-3"
      role="img"
      :aria-label="ariaLabel"
    >
      <div v-if="hasData" class="space-y-3">
        <div v-for="item in rows" :key="item.id" class="space-y-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span class="truncate font-medium text-foreground" :title="item.name">{{ item.label }}</span>
            <span class="shrink-0 text-muted-foreground">{{ item.duration }}</span>
          </div>
          <div class="h-3 overflow-hidden rounded-full border border-border/60 bg-white/6">
            <div
              class="h-full rounded-full bg-[linear-gradient(90deg,rgba(255,255,255,0.95),rgba(255,255,255,0.55))] transition-[width]"
              :style="{ width: item.width }"
            />
          </div>
        </div>
      </div>

      <div v-else class="flex h-full items-center justify-center text-sm text-muted-foreground">
        Нет данных по играм
      </div>
    </div>
  </section>
</template>
