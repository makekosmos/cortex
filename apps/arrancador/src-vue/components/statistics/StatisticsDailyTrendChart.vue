<script setup lang="ts">
import { computed } from "vue";
import { BarChart3 } from "lucide-vue-next";
import {
  formatDateLong,
  formatDateShort,
  formatDuration,
  type DailyTrendPoint,
} from "@vue-app/lib/statistics";

const props = defineProps<{
  data: DailyTrendPoint[];
  hasData: boolean;
  rangeLabel: string;
  ariaLabel: string;
}>();

const viewBoxWidth = 640;
const viewBoxHeight = 220;
const padding = {
  top: 16,
  right: 20,
  bottom: 34,
  left: 18,
};

const chartWidth = viewBoxWidth - padding.left - padding.right;
const chartHeight = viewBoxHeight - padding.top - padding.bottom;

const maxHours = computed(() => Math.max(...props.data.map((entry) => entry.hours), 1));

const points = computed(() => {
  if (props.data.length === 0) {
    return [];
  }

  if (props.data.length === 1) {
    return [
      {
        ...props.data[0],
        x: padding.left + chartWidth / 2,
        y: padding.top + chartHeight / 2,
      },
    ];
  }

  return props.data.map((entry, index) => ({
    ...entry,
    x: padding.left + (chartWidth * index) / (props.data.length - 1),
    y: padding.top + chartHeight - (entry.hours / maxHours.value) * chartHeight,
  }));
});

const linePath = computed(() => {
  if (points.value.length === 0) {
    return "";
  }

  if (points.value.length === 1) {
    const point = points.value[0];
    return `M ${point.x} ${point.y}`;
  }

  return points.value
    .map((point, index) => `${index === 0 ? "M" : "L"} ${point.x} ${point.y}`)
    .join(" ");
});

const areaPath = computed(() => {
  if (points.value.length === 0) {
    return "";
  }

  const baseline = padding.top + chartHeight;
  const line = linePath.value;
  const firstPoint = points.value[0];
  const lastPoint = points.value[points.value.length - 1];

  return `${line} L ${lastPoint.x} ${baseline} L ${firstPoint.x} ${baseline} Z`;
});

const yTicks = computed(() =>
  [0, 0.25, 0.5, 0.75, 1].map((ratio) => {
    const value = maxHours.value * ratio;
    return {
      id: ratio,
      label: `${Math.round(value * 10) / 10} ч`,
      y: padding.top + chartHeight - chartHeight * ratio,
    };
  }),
);

const xTicks = computed(() => {
  if (points.value.length <= 4) {
    return points.value;
  }

  const indexes = new Set([
    0,
    Math.floor((points.value.length - 1) * 0.33),
    Math.floor((points.value.length - 1) * 0.66),
    points.value.length - 1,
  ]);

  return [...indexes].sort((left, right) => left - right).map((index) => points.value[index]);
});

const peakPoint = computed(() =>
  props.data.reduce<DailyTrendPoint | null>((currentPeak, entry) => {
    if (!currentPeak || entry.seconds > currentPeak.seconds) {
      return entry;
    }
    return currentPeak;
  }, null),
);
</script>

<template>
  <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
    <div class="flex items-start justify-between gap-3">
      <div>
        <div class="flex items-center gap-2 text-sm font-[510]">
          <BarChart3 class="h-4 w-4 text-muted-foreground" />
          Динамика по дням
        </div>
        <p class="mt-1 text-sm text-muted-foreground">{{ rangeLabel }}</p>
      </div>
      <div v-if="peakPoint" class="text-right text-xs text-muted-foreground">
        <div>Пик</div>
        <div class="mt-1 font-medium text-foreground">
          {{ formatDateLong(peakPoint.date) }} · {{ formatDuration(peakPoint.seconds) }}
        </div>
      </div>
    </div>

    <div
      class="mt-4 h-[260px] overflow-hidden rounded-2xl border border-border/60 bg-background/35 p-3"
      role="img"
      :aria-label="ariaLabel"
    >
      <div v-if="hasData" class="h-full">
        <svg
          class="h-full w-full"
          :viewBox="`0 0 ${viewBoxWidth} ${viewBoxHeight}`"
          preserveAspectRatio="none"
        >
          <defs>
            <linearGradient id="statistics-daily-gradient" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="currentColor" stop-opacity="0.35" />
              <stop offset="100%" stop-color="currentColor" stop-opacity="0" />
            </linearGradient>
          </defs>

          <line
            v-for="tick in yTicks"
            :key="tick.id"
            :x1="padding.left"
            :x2="viewBoxWidth - padding.right"
            :y1="tick.y"
            :y2="tick.y"
            stroke="hsl(var(--border))"
            stroke-dasharray="4 4"
          />

          <path
            :d="areaPath"
            fill="url(#statistics-daily-gradient)"
            class="text-white"
          />
          <path
            :d="linePath"
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2.5"
            class="text-white"
          />

          <circle
            v-for="point in points"
            :key="point.date"
            :cx="point.x"
            :cy="point.y"
            r="3.5"
            fill="currentColor"
            class="text-white"
          />

          <text
            v-for="tick in yTicks"
            :key="`label-${tick.id}`"
            :x="viewBoxWidth - padding.right + 4"
            :y="tick.y + 4"
            fill="hsl(var(--muted-foreground))"
            font-size="11"
          >
            {{ tick.label }}
          </text>

          <text
            v-for="tick in xTicks"
            :key="`x-${tick.date}`"
            :x="tick.x"
            :y="viewBoxHeight - 8"
            fill="hsl(var(--muted-foreground))"
            font-size="11"
            text-anchor="middle"
          >
            {{ formatDateShort(tick.date) }}
          </text>
        </svg>
      </div>

      <div v-else class="flex h-full items-center justify-center text-sm text-muted-foreground">
        Нет данных за период
      </div>
    </div>
  </section>
</template>
