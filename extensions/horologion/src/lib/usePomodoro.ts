// @deprecated — после Wave 2 (2026-05-15) state machine живёт в kepler-backend
// (services/kepler-backend/src/pomodoro_host.rs + crates/ark-core/src/pomodoro/),
// а renderer использует thin wrapper `usePomodoroSession.ts`. Этот файл
// оставлен временно для возможного отката; см. .agent/tasks/
// 2026-05-15-pomodoro-backend-session/. PomodoroView / HomeView / main.ts
// больше не импортируют отсюда.

import { computed, ref } from "vue";
import { pomodoroSettings } from "./pomodoroSettings";
import { playSound } from "./sounds";
import { notifyEntriesChanged, pomodoroDraft } from "./store";

export type PomodoroPhase = "idle" | "work" | "shortBreak" | "longBreak";

interface PhaseContext {
  /** Описание для time_entry (берётся из draft либо «Помодоро»). */
  title: string;
  /** Список задач для work-сегмента. Время делится поровну на finish. */
  tasks: Array<{ id: string; title: string }>;
  /**
   * Опционально: override длительности work-фазы в минутах. Используется
   * ARK command bus'ом (`horologion:pomodoro:25` / `:50`).
   */
  workMinOverride?: number;
}

/**
 * Простой state-machine для Pomodoro-цикла. Singleton.
 */
function createPomodoroState() {
  const phase = ref<PomodoroPhase>("idle");
  const remainingMs = ref(0);
  const totalMs = ref(0);
  const completedPomodoros = ref(0);
  const isRunning = ref(false);
  const isPaused = ref(false);
  const currentEntryId = ref<string | null>(null);
  const lastContext = ref<PhaseContext | null>(null);

  let tickHandle: ReturnType<typeof setInterval> | null = null;
  let phaseEndsAt = 0;

  function durationMsForPhase(p: PomodoroPhase, ctx?: PhaseContext | null): number {
    if (p === "work") {
      const min = ctx?.workMinOverride ?? pomodoroSettings.workMin;
      return min * 60 * 1000;
    }
    if (p === "shortBreak") return pomodoroSettings.shortBreakMin * 60 * 1000;
    if (p === "longBreak") return pomodoroSettings.longBreakMin * 60 * 1000;
    return 0;
  }

  function nextPhaseAfter(p: PomodoroPhase): PomodoroPhase {
    if (p === "work") {
      return completedPomodoros.value + 1 >= pomodoroSettings.pomodorosUntilLongBreak
        ? "longBreak"
        : "shortBreak";
    }
    return "work";
  }

  function startTicker() {
    stopTicker();
    tickHandle = setInterval(() => {
      if (!isRunning.value || isPaused.value) return;
      remainingMs.value = Math.max(0, phaseEndsAt - Date.now());
      if (remainingMs.value <= 0) {
        void finishPhase();
      }
    }, 250);
  }

  function stopTicker() {
    if (tickHandle) {
      clearInterval(tickHandle);
      tickHandle = null;
    }
  }

  async function createArkEntry(p: PomodoroPhase, ctx: PhaseContext): Promise<string | null> {
    if (p === "work") {
      const firstTask = ctx.tasks[0];
      const entry = await window.horologion.timeEntries.startTimer({
        title: ctx.title || "Помодоро",
        taskId: firstTask?.id ?? null,
        taskTitle: firstTask?.title ?? null,
      });
      notifyEntriesChanged();
      return entry.id;
    }
    if (pomodoroSettings.trackBreaksAsRest) {
      const entry = await window.horologion.timeEntries.startTimer({
        title: "Отдых",
        taskId: null,
        taskTitle: null,
      });
      notifyEntriesChanged();
      return entry.id;
    }
    return null;
  }

  async function closeArkEntry() {
    const id = currentEntryId.value;
    if (!id) return;

    const liveTitle = pomodoroDraft.value.title;
    const liveTasks = pomodoroDraft.value.tasks.slice();
    const isWork = phase.value === "work";
    const effectiveTitle = (isWork ? liveTitle : lastContext.value?.title) || "Помодоро";
    const effectiveTasks = isWork ? liveTasks : lastContext.value?.tasks ?? [];

    if (isWork && effectiveTasks.length > 1) {
      try {
        const anchorList = await window.horologion.timeEntries.list();
        const anchor = anchorList.find((e) => e.id === id);
        if (anchor) {
          const startMs = new Date(anchor.startedAt).getTime();
          const endMs = Date.now();
          const total = Math.max(0, endMs - startMs);
          const n = effectiveTasks.length;
          const slotMs = total / n;

          await window.horologion.timeEntries.delete(id);

          for (let i = 0; i < n; i++) {
            const t = effectiveTasks[i];
            const sliceStart = new Date(startMs + i * slotMs).toISOString();
            const sliceEnd = new Date(startMs + (i + 1) * slotMs).toISOString();
            await window.horologion.timeEntries.create({
              title: effectiveTitle,
              startedAt: sliceStart,
              endedAt: sliceEnd,
              taskId: t.id,
              taskTitle: t.title,
            });
          }
        }
      } catch (e) {
        console.error("[pomodoro] failed to split multi-task entry:", e);
        try {
          await window.horologion.timeEntries.stopTimer(id);
        } catch {
          /* ignore */
        }
      }
    } else if (isWork) {
      const t = effectiveTasks[0] ?? null;
      try {
        await window.horologion.timeEntries.update({
          id,
          title: effectiveTitle,
          taskId: t?.id ?? null,
          taskTitle: t?.title ?? null,
        });
      } catch (e) {
        console.error("[pomodoro] failed to update work entry before stop:", e);
      }
      try {
        await window.horologion.timeEntries.stopTimer(id);
      } catch {
        /* sidecar мог уже закрыть запись */
      }
    } else {
      try {
        await window.horologion.timeEntries.stopTimer(id);
      } catch {
        /* sidecar мог уже закрыть запись */
      }
    }

    currentEntryId.value = null;
    notifyEntriesChanged();
  }

  function notifyEnd(p: PomodoroPhase) {
    if (p === "work") {
      playSound(pomodoroSettings.workEndSound);
    } else if (p === "shortBreak" || p === "longBreak") {
      playSound(pomodoroSettings.breakEndSound);
    }

    if (!pomodoroSettings.systemNotifications) return;
    try {
      if (typeof Notification === "undefined") return;
      if (Notification.permission !== "granted") return;
      const titles: Record<PomodoroPhase, string> = {
        idle: "",
        work: "Фокус закончен",
        shortBreak: "Перерыв закончен",
        longBreak: "Большой перерыв закончен",
      };
      const bodies: Record<PomodoroPhase, string> = {
        idle: "",
        work: "Время отдохнуть.",
        shortBreak: "Время вернуться к работе.",
        longBreak: "Хорошо отдохнули — продолжаем.",
      };
      new Notification(titles[p], { body: bodies[p] });
    } catch {
      /* ignore */
    }
  }

  async function startPhase(p: PomodoroPhase, ctx: PhaseContext) {
    phase.value = p;
    totalMs.value = durationMsForPhase(p, ctx);
    remainingMs.value = totalMs.value;
    isRunning.value = true;
    isPaused.value = false;
    phaseEndsAt = Date.now() + totalMs.value;
    lastContext.value = { ...ctx };

    await closeArkEntry();
    currentEntryId.value = await createArkEntry(p, ctx);
    startTicker();
  }

  async function finishPhase() {
    const finished = phase.value;
    stopTicker();
    isRunning.value = false;
    isPaused.value = false;
    remainingMs.value = 0;

    await closeArkEntry();

    if (finished === "work") {
      completedPomodoros.value += 1;
    }

    notifyEnd(finished);

    const next = nextPhaseAfter(finished);
    const ctx = lastContext.value ?? { title: "Помодоро", tasks: [] };

    const shouldAuto =
      (next === "work" && pomodoroSettings.autoStartWork) ||
      ((next === "shortBreak" || next === "longBreak") &&
        pomodoroSettings.autoStartBreak);

    if (shouldAuto) {
      await startPhase(next, ctx);
    } else {
      phase.value = next;
      totalMs.value = durationMsForPhase(next, ctx);
      remainingMs.value = totalMs.value;
      isRunning.value = false;
    }
  }

  async function start(ctx: PhaseContext) {
    const p: PomodoroPhase = phase.value === "idle" ? "work" : phase.value;
    await startPhase(p, ctx);
  }

  async function skip() {
    if (!isRunning.value && phase.value === "idle") return;
    if (isRunning.value) {
      remainingMs.value = 0;
      await finishPhase();
    } else {
      const next = nextPhaseAfter(phase.value);
      phase.value = next;
      totalMs.value = durationMsForPhase(next, lastContext.value);
      remainingMs.value = totalMs.value;
    }
  }

  async function stop() {
    stopTicker();
    isRunning.value = false;
    isPaused.value = false;
    phase.value = "idle";
    remainingMs.value = 0;
    totalMs.value = 0;
    completedPomodoros.value = 0;
    await closeArkEntry();
  }

  function pause() {
    if (!isRunning.value || isPaused.value) return;
    isPaused.value = true;
  }

  function resume() {
    if (!isPaused.value) return;
    isPaused.value = false;
    phaseEndsAt = Date.now() + remainingMs.value;
  }

  if (
    pomodoroSettings.systemNotifications &&
    typeof Notification !== "undefined" &&
    Notification.permission === "default"
  ) {
    void Notification.requestPermission().catch(() => {});
  }

  return {
    phase,
    remainingMs,
    totalMs,
    completedPomodoros,
    isRunning,
    isPaused,
    progress: computed(() =>
      totalMs.value > 0 ? 1 - remainingMs.value / totalMs.value : 0,
    ),
    start,
    pause,
    resume,
    skip,
    stop,
    dispose() {
      stopTicker();
    },
  };
}

const pomodoro = createPomodoroState();

export function usePomodoro() {
  return pomodoro;
}
