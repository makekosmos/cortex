<script setup lang="ts">
import { computed } from "vue";
import type { CoderDifficultyStats } from "./types";

const props = defineProps<{ stats: CoderDifficultyStats }>();

const segments = computed(() => {
  let offset = 0;
  return [
    { key: "easy", label: "Лёгкие", count: props.stats.easy },
    { key: "medium", label: "Средние", count: props.stats.medium },
    { key: "hard", label: "Сложные", count: props.stats.hard },
  ]
    .map((segment, index) => {
      const share = props.stats.total > 0 ? (segment.count / props.stats.total) * 100 : 0;
      const result = {
        ...segment,
        style: {
          strokeDasharray: `${Math.max(0, share - 1.5)} ${100 - Math.max(0, share - 1.5)}`,
          strokeDashoffset: `${-(offset + 0.75)}`,
          animationDelay: `${index * 100}ms`,
        },
      };
      offset += share;
      return result;
    })
    .filter((segment) => segment.count > 0);
});
</script>

<template>
  <section class="difficulty-gauge" aria-label="Уровень алгоритмов">
    <div class="difficulty-copy">
      <h2 class="difficulty-title">Уровень алгоритмов</h2>
      <p class="difficulty-description">По уникальным принятым решениям LeetCode</p>
    </div>
    <div class="gauge-chart">
      <svg viewBox="0 0 200 112" role="img" aria-label="Распределение сложности задач">
        <path class="gauge-track" pathLength="100" d="M 20 100 A 80 80 0 0 1 180 100" />
        <path
          v-for="segment in segments"
          :key="segment.key"
          :class="['gauge-segment', `gauge-segment--${segment.key}`]"
          :style="segment.style"
          pathLength="100"
          d="M 20 100 A 80 80 0 0 1 180 100"
        >
          <title>{{ segment.label }}: {{ segment.count }}</title>
        </path>
      </svg>
      <div class="gauge-value">
        <strong v-if="stats.available > 0">{{ stats.total }}</strong>
        <strong v-else>Нет данных</strong>
        <span>Решено</span>
      </div>
    </div>
    <div class="difficulty-legend">
      <div>
        <i class="difficulty-dot difficulty-dot--easy" /><span>Лёгкие</span
        ><strong>{{ stats.easy }}/{{ stats.easyTotal }}</strong>
      </div>
      <div>
        <i class="difficulty-dot difficulty-dot--medium" /><span>Средние</span
        ><strong>{{ stats.medium }}/{{ stats.mediumTotal }}</strong>
      </div>
      <div>
        <i class="difficulty-dot difficulty-dot--hard" /><span>Сложные</span
        ><strong>{{ stats.hard }}/{{ stats.hardTotal }}</strong>
      </div>
    </div>
  </section>
</template>

<style scoped>
.difficulty-gauge {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 210px minmax(150px, 0.7fr);
  align-items: center;
  gap: 18px;
  margin-bottom: 18px;
  padding: 16px 0;
  border-top: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
}

.difficulty-title,
.difficulty-description {
  margin: 0;
}

.difficulty-title {
  color: var(--foreground);
  font-size: 0.875rem;
}

.difficulty-description {
  margin-top: 5px;
  color: var(--muted-foreground);
  font-size: 0.6875rem;
  line-height: 1.45;
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

.gauge-segment--easy {
  stroke: var(--coder-easy);
}
.gauge-segment--medium {
  stroke: var(--coder-medium);
}
.gauge-segment--hard {
  stroke: var(--coder-hard);
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
  font-size: 1rem;
}

.gauge-value span,
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

.difficulty-dot--easy {
  background: var(--coder-easy);
}
.difficulty-dot--medium {
  background: var(--coder-medium);
}
.difficulty-dot--hard {
  background: var(--coder-hard);
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
