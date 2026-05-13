<script setup lang="ts">
import { computed } from "vue";
import { Play, Pause, SkipForward, Square } from "lucide-vue-next";
import { useRouter } from "vue-router";
import { usePomodoro, type PomodoroPhase } from "../lib/usePomodoro";
import { pomodoroDraft, type PomodoroDraftTask } from "../lib/store";
import { pomodoroSettings } from "../lib/pomodoroSettings";
import PomodoroDraftInput from "../components/PomodoroDraftInput.vue";

const router = useRouter();
const p = usePomodoro();

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

const draftV = computed({
  get: () => pomodoroDraft.value.title,
  set: (v) => {
    pomodoroDraft.value = { ...pomodoroDraft.value, title: v };
  },
});
const draftTasks = computed({
  get: () => pomodoroDraft.value.tasks,
  set: (v: PomodoroDraftTask[]) => {
    pomodoroDraft.value = { ...pomodoroDraft.value, tasks: v };
  },
});

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
    <!-- Описание + чипы выбранных задач. Можно выбрать несколько — на finish
         time work-сегмента делится поровну между ними. -->
    <PomodoroDraftInput
      v-model="draftV"
      v-model:tasks="draftTasks"
      placeholder="Над чем работаем? @ для задачи"
      class="pomo__draft"
      @submit="onPrimary"
    />

    <div class="pomo__ring-wrap">
      <svg class="pomo__ring" viewBox="0 0 280 280" width="280" height="280">
        <circle class="pomo__ring-bg" cx="140" cy="140" :r="R" />
        <!-- 60 делений = 60 минут (1 деление = 1 минута). Сдвинуты внутрь
             кольца: outer край в R-9, inner край в R-15 для обычных делений;
             major (каждое 5-е, 5-минутная отметка) удлинено внутрь до R-18. -->
        <g class="pomo__ticks">
          <line
            v-for="i in 60"
            :key="i"
            :x1="140 + (i % 5 === 1 ? R - 18 : R - 15) * Math.cos(((i - 1) * 6 - 90) * (Math.PI / 180))"
            :y1="140 + (i % 5 === 1 ? R - 18 : R - 15) * Math.sin(((i - 1) * 6 - 90) * (Math.PI / 180))"
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
        <span class="pomo__phase">{{ phaseLabel }}</span>
        <span class="pomo__time">{{ minutesLabel }}</span>
        <span class="pomo__status">{{ statusLabel }}</span>
      </div>
    </div>

    <div class="pomo__dots" :title="`${pomodorosDone} из ${pomodorosTotal}`">
      <span
        v-for="i in pomodorosTotal"
        :key="i"
        class="pomo__dot"
        :class="{ 'pomo__dot--done': i <= pomodorosDone }"
      />
    </div>

    <div class="pomo__actions">
      <button class="pomo__primary" @click="onPrimary">
        <Pause v-if="p.isRunning.value && !p.isPaused.value" :size="16" :stroke-width="2.2" />
        <Play v-else :size="16" :stroke-width="2.2" />
        {{ primaryLabel }}
      </button>
      <div
        v-if="p.isRunning.value || p.phase.value !== 'idle'"
        class="pomo__secondary"
      >
        <button class="pomo__secbtn" @click="p.skip">
          <SkipForward :size="14" :stroke-width="1.8" />
          Пропустить
        </button>
        <button class="pomo__secbtn" @click="p.stop">
          <Square :size="14" :stroke-width="1.8" />
          Стоп
        </button>
      </div>
    </div>

    <button
      v-if="!p.isRunning.value"
      class="pomo__settings-link"
      @click="router.push('/settings')"
    >
      Настройки помодоро
    </button>
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

.pomo__dots {
  display: flex;
  gap: 6px;
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
  border-radius: calc(var(--radius) * 0.75);
  corner-shape: var(--corner-shape);
  background: var(--accent);
  color: var(--accent-foreground);
  font-family: inherit;
  font-size: 0.875rem;
  font-weight: 600;
  cursor: pointer;
  transition:
    background-color 220ms cubic-bezier(0.2, 0, 0, 1),
    color 220ms cubic-bezier(0.2, 0, 0, 1);
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
  border-radius: calc(var(--radius) * 0.6);
  corner-shape: var(--corner-shape);
  font-family: inherit;
  font-size: 0.8125rem;
  font-weight: 500;
  cursor: pointer;
  transition:
    background-color 160ms cubic-bezier(0.2, 0, 0, 1),
    color 160ms cubic-bezier(0.2, 0, 0, 1);
}

.pomo__secbtn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.pomo__settings-link {
  background: transparent;
  border: none;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  font-family: inherit;
  font-size: 0.8125rem;
  cursor: pointer;
  text-decoration: underline;
}

.pomo__settings-link:hover {
  color: var(--foreground);
}
</style>
