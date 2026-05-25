<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { Play, Pause, SkipForward, Square, Settings } from "@lucide/vue";
import { usePomodoroSession as usePomodoro, type PomodoroPhase } from "../lib/usePomodoroSession";
import { pomodoroDraft } from "../lib/store";
import { pomodoroSettings } from "../lib/pomodoroSettings";

const p = usePomodoro();
const router = useRouter();

function openSettings() {
  void router.push("/settings");
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

const minutesLabel = computed(() => {
  if (p.phase.value === "idle" && !p.isRunning.value) {
    return `${pad(pomodoroSettings.workMin)}:00`;
  }
  const total = Math.max(0, Math.ceil(p.remainingMs.value / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${pad(m)}:${pad(s)}`;
});

const phaseLabel = computed<string>(() => {
  const map: Record<PomodoroPhase, string> = {
    idle: "Готов",
    work: "Фокус",
    shortBreak: "Перерыв",
    longBreak: "Большой перерыв",
  };
  return map[p.phase.value];
});

const statusLabel = computed<string>(() => {
  if (p.phase.value === "idle") return "Готов?";
  if (p.isRunning.value && !p.isPaused.value) return "Идёт…";
  if (p.isPaused.value) return "Пауза";
  return "Готов?";
});

const R = 120;
const CIRC = 2 * Math.PI * R;
const dashOffset = computed(() => CIRC * (1 - p.progress.value));

const ringColorClass = computed(() => {
  if (p.phase.value === "work") return "ring--work";
  if (p.phase.value === "shortBreak" || p.phase.value === "longBreak") return "ring--break";
  return "ring--idle";
});

const pomodorosDone = computed(() => p.completedPomodoros.value);
const pomodorosTotal = computed(() => pomodoroSettings.pomodorosUntilLongBreak);
const todayCompleted = computed(() => p.todayCompleted.value);

async function onPrimary() {
  if (p.isRunning.value) {
    if (p.isPaused.value) p.resume();
    else p.pause();
    return;
  }
  await p.start({
    title: pomodoroDraft.value.title,
    tasks: pomodoroDraft.value.tasks.slice(),
  });
}

const primaryLabel = computed(() => {
  if (!p.isRunning.value && p.phase.value === "idle") return "Начать сессию";
  if (p.isRunning.value && !p.isPaused.value) return "Пауза";
  if (p.isPaused.value) return "Продолжить";
  if (p.phase.value === "work") return "Старт фокуса";
  return "Старт перерыва";
});
</script>

<template>
  <div class="pomo">
    <div class="pomo__ring-wrap">
      <svg class="pomo__ring" viewBox="0 0 280 280" width="280" height="280">
        <circle class="pomo__ring-bg" cx="140" cy="140" :r="R" />
        <g class="pomo__ticks">
          <line
            v-for="i in 60"
            :key="i"
            :x1="
              140 + (i % 5 === 1 ? R - 18 : R - 15) * Math.cos(((i - 1) * 6 - 90) * (Math.PI / 180))
            "
            :y1="
              140 + (i % 5 === 1 ? R - 18 : R - 15) * Math.sin(((i - 1) * 6 - 90) * (Math.PI / 180))
            "
            :x2="140 + (R - 9) * Math.cos(((i - 1) * 6 - 90) * (Math.PI / 180))"
            :y2="140 + (R - 9) * Math.sin(((i - 1) * 6 - 90) * (Math.PI / 180))"
            :class="i % 5 === 1 ? 'pomo__tick pomo__tick--major' : 'pomo__tick'"
          />
        </g>
        <circle
          class="pomo__ring-fg"
          :class="ringColorClass"
          cx="140"
          cy="140"
          :r="R"
          :stroke-dasharray="CIRC"
          :stroke-dashoffset="dashOffset"
        />
      </svg>
      <div class="pomo__center">
        <span v-if="p.phase.value !== 'idle'" class="pomo__phase">{{ phaseLabel }}</span>
        <span class="pomo__time">{{ minutesLabel }}</span>
        <span v-if="p.isRunning.value" class="pomo__status">{{ statusLabel }}</span>
      </div>
    </div>

    <div class="pomo__progress">
      <div class="pomo__dots" :title="`${pomodorosDone} из ${pomodorosTotal} до длинного перерыва`">
        <span
          v-for="i in pomodorosTotal"
          :key="i"
          class="pomo__dot"
          :class="{ 'pomo__dot--done': i <= pomodorosDone }"
        />
      </div>
      <span class="pomo__today" :title="'Завершённых помодоров за сегодня'">
        Сегодня: {{ todayCompleted }}
      </span>
    </div>

    <div class="pomo__actions">
      <button class="pomo__primary" @click="onPrimary">
        <Pause v-if="p.isRunning.value && !p.isPaused.value" :size="16" :stroke-width="2.2" />
        <Play v-else :size="16" :stroke-width="2.2" />
        {{ primaryLabel }}
      </button>
      <div v-if="p.isRunning.value || p.phase.value !== 'idle'" class="pomo__secondary">
        <button class="pomo__secbtn" @click="p.skip">
          <SkipForward :size="14" :stroke-width="1.8" />
          Пропустить
        </button>
        <button class="pomo__secbtn" @click="p.stop">
          <Square :size="14" :stroke-width="1.8" />
          Стоп
        </button>
      </div>

      <div v-if="!p.isRunning.value" class="pomo__secondary">
        <button class="pomo__secbtn" @click="openSettings">
          <Settings :size="14" :stroke-width="1.8" />
          Настройки помодоро
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.pomo {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

.pomo__draft {
  width: 280px;
  max-width: 100%;
}

.pomo__ring-wrap {
  position: relative;
  width: 280px;
  height: 280px;
}

.pomo__ring {
  width: 100%;
  height: 100%;
  transform: rotate(-90deg);
  display: block;
}

.pomo__ring-bg {
  fill: none;
  stroke: color-mix(in srgb, var(--foreground) 8%, transparent);
  stroke-width: 10;
}

.pomo__ring-fg {
  fill: none;
  stroke-width: 10;
  stroke-linecap: round;
  transition: stroke-dashoffset 700ms cubic-bezier(0.2, 0, 0, 1);
}

.pomo__ring-fg.ring--work {
  stroke: var(--accent);
}

.pomo__ring-fg.ring--break {
  stroke: var(--status-success);
}

.pomo__ring-fg.ring--idle {
  stroke: var(--accent);
}

.pomo__ticks line {
  stroke: color-mix(in srgb, var(--foreground) 18%, transparent);
  stroke-width: 1;
}

.pomo__ticks .pomo__tick--major {
  stroke: color-mix(in srgb, var(--foreground) 38%, transparent);
  stroke-width: 1.5;
}

.pomo__center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.25rem;
  pointer-events: none;
}

.pomo__phase {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
  font-weight: 600;
}

.pomo__time {
  font-family: var(--font-mono);
  font-size: 3.25rem;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--foreground);
  letter-spacing: -0.02em;
  line-height: 1;
}

.pomo__status {
  font-size: 0.8125rem;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
}

.pomo__progress {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.pomo__dots {
  display: flex;
  gap: 6px;
}

.pomo__today {
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  letter-spacing: 0.02em;
}

.pomo__dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--foreground) 15%, transparent);
  transition: background-color 220ms cubic-bezier(0.2, 0, 0, 1);
}

.pomo__dot--done {
  background: var(--accent);
}

.pomo__actions {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  max-width: 280px;
}

.pomo__primary {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  width: 100%;
  height: 40px;
  border: none;
  border-radius: var(--radius-button);
  corner-shape: var(--corner-shape);
  background: var(--accent);
  color: var(--accent-foreground);
  font-family: inherit;
  font-size: 0.875rem;
  font-weight: 600;
  transition:
    background-color 350ms cubic-bezier(0.2, 0, 0, 1),
    color 350ms cubic-bezier(0.2, 0, 0, 1);
}

.pomo__primary:hover {
  background: var(--foreground);
  color: var(--background);
}

.pomo__secondary {
  display: flex;
  width: 100%;
  gap: 0.5rem;
}

.pomo__secbtn {
  flex: 1 1 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  height: 36px;
  padding: 0 0.75rem;
  border: 1px solid var(--border);
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 75%, transparent);
  border-radius: calc(var(--radius) * 1.5);
  corner-shape: var(--corner-shape);
  font-family: inherit;
  font-size: 0.8125rem;
  font-weight: 500;
  transition:
    background-color 350ms cubic-bezier(0.2, 0, 0, 1),
    color 350ms cubic-bezier(0.2, 0, 0, 1);
}

.pomo__secbtn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}
</style>
