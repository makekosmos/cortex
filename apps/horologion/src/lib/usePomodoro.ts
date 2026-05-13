import { computed, onBeforeUnmount, ref } from "vue";
import { pomodoroSettings } from "./pomodoroSettings";
import { playSound } from "./sounds";
import { notifyEntriesChanged } from "./store";

export type PomodoroPhase = "idle" | "work" | "shortBreak" | "longBreak";

interface PhaseContext {
  /** Описание для time_entry (берётся из draft либо «Помодоро»). */
  title: string;
  /** Список задач для work-сегмента. Время делится поровну на finish. */
  tasks: Array<{ id: string; title: string }>;
}

/**
 * Простой state-machine для Pomodoro-цикла.
 *
 * Один singleton-инстанс на всё приложение — `PomodoroView` и `App` шарят
 * состояние через прямой импорт хранилища.
 */
function createPomodoroState() {
  const phase = ref<PomodoroPhase>("idle");
  const remainingMs = ref(0);
  const totalMs = ref(0);
  const completedPomodoros = ref(0);
  const isRunning = ref(false);
  const isPaused = ref(false);
  /** ARK id текущей записи, если фаза создавала time_entry. */
  const currentEntryId = ref<string | null>(null);
  const lastContext = ref<PhaseContext | null>(null);

  let tickHandle: ReturnType<typeof setInterval> | null = null;
  let phaseEndsAt = 0;

  function durationMsForPhase(p: PomodoroPhase): number {
    if (p === "work") return pomodoroSettings.workMin * 60 * 1000;
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
      // Якорная запись — пока work идёт, отображается в top-bar как running.
      // На close её заменяем на N split-entries (по одной на задачу).
      const firstTask = ctx.tasks[0];
      const entry = await window.horologion.timeEntries.startTimer({
        title: ctx.title || "Помодоро",
        taskId: firstTask?.id ?? null,
        taskTitle: firstTask?.title ?? null,
      });
      notifyEntriesChanged();
      return entry.id;
    }
    // break-фаза
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
    const ctx = lastContext.value;

    // Work-фаза с несколькими задачами → split на N равных промежутков.
    if (phase.value === "work" && ctx && ctx.tasks.length > 1) {
      try {
        // Найдём anchor-entry, заберём его startedAt, удалим, создадим N entries.
        const anchorList = await window.horologion.timeEntries.list();
        const anchor = anchorList.find((e) => e.id === id);
        if (anchor) {
          const startMs = new Date(anchor.startedAt).getTime();
          const endMs = Date.now();
          const totalMs = Math.max(0, endMs - startMs);
          const n = ctx.tasks.length;
          const slotMs = totalMs / n;

          // Удаляем якорь
          await window.horologion.timeEntries.delete(id);

          // Создаём N split-entries
          for (let i = 0; i < n; i++) {
            const t = ctx.tasks[i];
            const sliceStart = new Date(startMs + i * slotMs).toISOString();
            const sliceEnd = new Date(startMs + (i + 1) * slotMs).toISOString();
            await window.horologion.timeEntries.create({
              title: ctx.title || "Помодоро",
              startedAt: sliceStart,
              endedAt: sliceEnd,
              taskId: t.id,
              taskTitle: t.title,
            });
          }
        }
      } catch (e) {
        console.error("[pomodoro] failed to split multi-task entry:", e);
        // Fallback: пробуем стопнуть якорь
        try {
          await window.horologion.timeEntries.stopTimer(id);
        } catch {
          /* ignore */
        }
      }
    } else {
      // Single task / break — обычный stop
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
    // Звук
    if (p === "work") {
      playSound(pomodoroSettings.workEndSound);
    } else if (p === "shortBreak" || p === "longBreak") {
      playSound(pomodoroSettings.breakEndSound);
    }

    // Системные уведомления (если разрешены настройкой + получено permission)
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
    totalMs.value = durationMsForPhase(p);
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

    // Auto-start или ждём явного клика?
    const shouldAuto =
      (next === "work" && pomodoroSettings.autoStartWork) ||
      ((next === "shortBreak" || next === "longBreak") &&
        pomodoroSettings.autoStartBreak);

    if (shouldAuto) {
      await startPhase(next, ctx);
    } else {
      // Готовы к следующей фазе, но ждём явного `start`.
      phase.value = next;
      totalMs.value = durationMsForPhase(next);
      remainingMs.value = totalMs.value;
      isRunning.value = false;
    }
  }

  async function start(ctx: PhaseContext) {
    // Если фаза уже выбрана (после finishPhase'а в non-auto режиме) — стартуем её.
    // Иначе начинаем с work.
    const p: PomodoroPhase = phase.value === "idle" ? "work" : phase.value;
    await startPhase(p, ctx);
  }

  async function skip() {
    // Прерываем фазу мгновенно — finishPhase отработает auto-start логику.
    if (!isRunning.value && phase.value === "idle") return;
    if (isRunning.value) {
      remainingMs.value = 0;
      await finishPhase();
    } else {
      // Уже в "готова к запуску" — пропустим к следующей фазе.
      const next = nextPhaseAfter(phase.value);
      phase.value = next;
      totalMs.value = durationMsForPhase(next);
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
    // Сохраняем оставшееся время — phaseEndsAt не сдвигаем, при resume пересчитаем.
  }

  function resume() {
    if (!isPaused.value) return;
    isPaused.value = false;
    phaseEndsAt = Date.now() + remainingMs.value;
  }

  // Запрашиваем разрешение на уведомления один раз (если настройка включена).
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
    /** Hook для размонтирования (на случай ручной очистки). */
    dispose() {
      stopTicker();
    },
  };
}

const pomodoro = createPomodoroState();

export function usePomodoro() {
  // composable не делает ничего нового при каждом вызове — возвращаем singleton.
  // onBeforeUnmount нам не нужен (singleton живёт всё время сессии).
  void onBeforeUnmount;
  return pomodoro;
}
