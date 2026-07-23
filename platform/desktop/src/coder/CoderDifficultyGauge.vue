<script setup lang="ts">
import { computed } from "vue";
import type { CoderBreakdownItem, CoderDifficultyStats } from "./types";

const props = defineProps<{ stats?: CoderDifficultyStats; ranks?: CoderBreakdownItem[] }>();

const kyuColor = (level: number) => {
  if (level >= 7) return "var(--foreground)";
  if (level >= 5) return "var(--status-warning)";
  if (level >= 3) return "var(--accent)";
  return "color-mix(in srgb, var(--accent) 68%, var(--destructive))";
};

const gauge = computed(() => {
  const source = props.ranks
    ? props.ranks.map((rank) => {
        const level = Number.parseInt(rank.label);
        return {
          key: `kyu-${level}`,
          label: rank.label,
          count: rank.count,
          value: String(rank.count),
          color: kyuColor(level),
        };
      })
    : [
        {
          key: "easy",
          label: "Лёгкие",
          count: props.stats?.easy ?? 0,
          value: `${props.stats?.easy ?? 0}/${props.stats?.easyTotal ?? 0}`,
          color: "var(--coder-easy)",
        },
        {
          key: "medium",
          label: "Средние",
          count: props.stats?.medium ?? 0,
          value: `${props.stats?.medium ?? 0}/${props.stats?.mediumTotal ?? 0}`,
          color: "var(--coder-medium)",
        },
        {
          key: "hard",
          label: "Сложные",
          count: props.stats?.hard ?? 0,
          value: `${props.stats?.hard ?? 0}/${props.stats?.hardTotal ?? 0}`,
          color: "var(--coder-hard)",
        },
      ];
  const total = props.ranks
    ? source.reduce((sum, segment) => sum + segment.count, 0)
    : (props.stats?.total ?? 0);
  let offset = 0;
  const segments = source
    .map((segment, index) => {
      const share = total > 0 ? (segment.count / total) * 100 : 0;
      const visibleShare = Math.max(0, share - Math.min(1.5, share * 0.35));
      const result = {
        ...segment,
        style: {
          stroke: segment.color,
          strokeDasharray: `${visibleShare} ${100 - visibleShare}`,
          strokeDashoffset: `${-(offset + 0.75)}`,
          animationDelay: `${index * 100}ms`,
        },
      };
      offset += share;
      return result;
    })
    .filter((segment) => segment.count > 0);
  return {
    ariaLabel: props.ranks ? "Распределение kata по kyu" : "Распределение сложности задач",
    hasData: props.ranks ? total > 0 : (props.stats?.available ?? 0) > 0,
    legend: source,
    segments,
    total,
  };
});
</script>

<template>
  <section class="difficulty-gauge" :aria-label="gauge.ariaLabel">
    <div class="gauge-chart">
      <svg viewBox="0 0 200 112" role="img" :aria-label="gauge.ariaLabel">
        <path class="gauge-track" pathLength="100" d="M 20 100 A 80 80 0 0 1 180 100" />
        <path
          v-for="segment in gauge.segments"
          :key="segment.key"
          class="gauge-segment"
          :style="segment.style"
          pathLength="100"
          d="M 20 100 A 80 80 0 0 1 180 100"
        >
          <title>{{ segment.label }}: {{ segment.count }}</title>
        </path>
      </svg>
      <div class="gauge-value">
        <strong v-if="gauge.hasData">{{ gauge.total }}</strong>
        <strong v-else>Нет данных</strong>
      </div>
    </div>
    <div class="difficulty-legend">
      <div v-for="segment in gauge.legend" :key="segment.key">
        <i class="difficulty-dot" :style="{ background: segment.color }" />
        <span>{{ segment.label }}</span>
        <strong>{{ segment.value }}</strong>
      </div>
    </div>
  </section>
</template>

<style scoped>
.difficulty-gauge {
  display: grid;
  grid-template-columns: 210px minmax(150px, 0.7fr);
  align-items: center;
  justify-content: center;
  gap: 18px;
  margin-bottom: 18px;
  padding: 16px 0;
  border-top: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
}

.gauge-chart {
  position: relative;
  height: 112px;
}

.gauge-chart svg {
  width: 100%;
  height: 100%;
  overflow: visible;
}

.gauge-track,
.gauge-segment {
  fill: none;
  stroke-width: 13;
  stroke-linecap: round;
}

.gauge-track {
  stroke: color-mix(in srgb, var(--foreground) 7%, transparent);
}

.gauge-segment {
  animation: gauge-reveal 700ms cubic-bezier(0.22, 1, 0.36, 1) both;
}

.gauge-value {
  position: absolute;
  right: 0;
  bottom: 3px;
  left: 0;
  display: flex;
  align-items: center;
  flex-direction: column;
}

.gauge-value strong {
  color: var(--foreground);
  font-size: 1.5rem;
}

.difficulty-legend span {
  color: var(--muted-foreground);
  font-size: 0.6875rem;
}

.difficulty-legend {
  display: flex;
  flex-direction: column;
  gap: 9px;
}

.difficulty-legend > div {
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr) auto;
  align-items: center;
  gap: 7px;
}

.difficulty-legend strong {
  color: var(--foreground);
  font-size: 0.75rem;
}

.difficulty-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}

@keyframes gauge-reveal {
  from {
    stroke-dasharray: 0 100;
  }
}

@media (prefers-reduced-motion: reduce) {
  .gauge-segment {
    animation: none;
  }
}

@media (max-width: 640px) {
  .difficulty-gauge {
    grid-template-columns: 1fr;
  }
  .gauge-chart {
    width: min(100%, 240px);
    margin: 0 auto;
  }
}
</style>
