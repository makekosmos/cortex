// Thin wrapper над backend'ovым pomodoro session (Wave 2).
//
// Заменяет `usePomodoro.ts` для PomodoroView и HomeView. Та же reactive
// shape (phase / remainingMs / totalMs / completedPomodoros / isRunning /
// isPaused / progress), но state живёт в kepler-backend'е (PomodoroHost),
// renderer — только subscriber.
//
// Контракт:
//   * onMounted (lazy init на первом обращении): fetch get_state, subscribe
//     к pomodoro_tick / pomodoro_phase_changed / pomodoro_finished.
//   * start(ctx) → ARK op `pomodoro.start` с config из pomodoroSettings.
//   * pause / resume / skip / stop — соответствующие ARK ops.
//   * Side effects (time_entry CRUD, audio, notifications) — слушает
//     pomodoro_phase_changed event и делает upsert через
//     window.horologion.timeEntries.* (как usePomodoro раньше).

import { computed, ref } from "vue";
import { pomodoroSettings } from "./pomodoroSettings";
import { playSound } from "./sounds";
import { notifyEntriesChanged, pomodoroDraft } from "./store";

export type PomodoroPhase = "idle" | "work" | "shortBreak" | "longBreak";

interface PhaseContext {
  title: string;
  tasks: Array<{ id: string; title: string }>;
  workMinOverride?: number;
}

interface BackendState {
  phase: PomodoroPhase;
  remainingMs: number;
  totalMs: number;
  completedPomodoros: number;
  isRunning: boolean;
  isPaused: boolean;
  title?: string;
  tasks?: Array<{ id: string; title: string }>;
  /** Wallclock (Unix ms) когда фаза кончится; null/undefined когда idle/paused. */
  phaseEndsAtMs?: number | null;
}

/** Smooth local interpolation cadence — 30fps достаточно для глаза, дешёво для CPU. */
const INTERP_INTERVAL_MS = 33;

interface KeplerArk {
  request: <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;
  subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
}

function kepler(): KeplerArk {
  const k = (window as unknown as { kepler?: { ark: KeplerArk } }).kepler;
  if (!k) throw new Error("window.kepler.ark not available");
  return k.ark;
}

function createSessionState() {
  const phase = ref<PomodoroPhase>("idle");
  const remainingMs = ref(0);
  const totalMs = ref(0);
  const completedPomodoros = ref(0);
  const isRunning = ref(false);
  const isPaused = ref(false);

  // Side-effect state — owned by renderer (как раньше в usePomodoro).
  const currentEntryId = ref<string | null>(null);
  const lastContext = ref<PhaseContext | null>(null);

  // Wallclock anchor для local interpolation. `null` ⇒ idle/paused,
  // remainingMs управляется server'ом напрямую.
  let phaseEndsAtMs: number | null = null;
  let interpTimer: ReturnType<typeof setInterval> | null = null;

  let initialised = false;
  let unsubFns: Array<() => void> = [];

  function recomputeFromAnchor(): void {
    if (phaseEndsAtMs == null) return;
    const next = Math.max(0, phaseEndsAtMs - Date.now());
    if (remainingMs.value !== next) remainingMs.value = next;
  }

  function ensureInterpTimer(): void {
    if (interpTimer != null) return;
    interpTimer = setInterval(recomputeFromAnchor, INTERP_INTERVAL_MS);
  }

  function clearInterpTimer(): void {
    if (interpTimer != null) {
      clearInterval(interpTimer);
      interpTimer = null;
    }
  }

  function applyState(s: BackendState): void {
    phase.value = s.phase;
    totalMs.value = s.totalMs;
    completedPomodoros.value = s.completedPomodoros;
    isRunning.value = s.isRunning;
    isPaused.value = s.isPaused;

    const anchor = s.phaseEndsAtMs ?? null;
    if (anchor != null && s.isRunning && !s.isPaused) {
      // Running phase → setup wallclock anchor; локальный interval двигает remainingMs.
      phaseEndsAtMs = anchor;
      remainingMs.value = Math.max(0, anchor - Date.now());
      ensureInterpTimer();
    } else {
      // Idle / paused / finished — server-driven remainingMs (frozen / 0).
      phaseEndsAtMs = null;
      clearInterpTimer();
      remainingMs.value = s.remainingMs;
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

  async function closeArkEntry(finishedPhase: PomodoroPhase): Promise<void> {
    const id = currentEntryId.value;
    if (!id) return;

    const liveTitle = pomodoroDraft.value.title;
    const liveTasks = pomodoroDraft.value.tasks.slice();
    const isWork = finishedPhase === "work";
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
            const t = effectiveTasks[i]!;
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
        console.error("[pomodoroSession] failed multi-task split:", e);
        try {
          await window.horologion.timeEntries.stopTimer(id);
        } catch { /* ignore */ }
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
        console.error("[pomodoroSession] failed update before stop:", e);
      }
      try {
        await window.horologion.timeEntries.stopTimer(id);
      } catch { /* sidecar мог уже закрыть */ }
    } else {
      try {
        await window.horologion.timeEntries.stopTimer(id);
      } catch { /* sidecar мог уже закрыть */ }
    }

    currentEntryId.value = null;
    notifyEntriesChanged();
  }

  function notifyEnd(p: PomodoroPhase) {
    if (p === "work") playSound(pomodoroSettings.workEndSound);
    else if (p === "shortBreak" || p === "longBreak") playSound(pomodoroSettings.breakEndSound);

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
    } catch { /* ignore */ }
  }

  async function ensureInit(): Promise<void> {
    if (initialised) return;
    initialised = true;

    try {
      const s = await kepler().request<BackendState>("pomodoro.get_state");
      applyState(s);
    } catch (e) {
      console.error("[pomodoroSession] get_state failed:", e);
    }

    unsubFns.push(
      kepler().subscribe("pomodoro_tick", (payload) => {
        applyState(payload as BackendState);
      }),
    );
    unsubFns.push(
      kepler().subscribe("pomodoro_phase_changed", (payload) => {
        const s = payload as BackendState & { from?: PomodoroPhase; to?: PomodoroPhase };
        const from = s.from ?? "idle";
        const to = s.to ?? s.phase;
        applyState(s);

        // Side-effect: при смене work-фазы → закрыть прошлый time_entry, открыть новый.
        // Если current → idle (stop) — просто close + reset.
        void (async () => {
          if (from === "work" || (from !== "idle" && pomodoroSettings.trackBreaksAsRest)) {
            await closeArkEntry(from);
          }
          if (to === "idle") {
            currentEntryId.value = null;
            return;
          }
          if (s.isRunning) {
            const ctx: PhaseContext = {
              title: s.title || lastContext.value?.title || "Помодоро",
              tasks: s.tasks ?? lastContext.value?.tasks ?? [],
            };
            lastContext.value = ctx;
            currentEntryId.value = await createArkEntry(to, ctx);
          }
        })();
      }),
    );
    unsubFns.push(
      kepler().subscribe("pomodoro_finished", (payload) => {
        const s = payload as BackendState & { finished?: PomodoroPhase };
        applyState(s);
        if (s.finished) notifyEnd(s.finished);
      }),
    );
  }

  function dispose(): void {
    for (const u of unsubFns) {
      try { u(); } catch { /* ignore */ }
    }
    unsubFns = [];
    clearInterpTimer();
    phaseEndsAtMs = null;
    initialised = false;
  }

  async function start(ctx: PhaseContext): Promise<void> {
    await ensureInit();
    lastContext.value = { ...ctx };
    const config = {
      workMin: pomodoroSettings.workMin,
      shortBreakMin: pomodoroSettings.shortBreakMin,
      longBreakMin: pomodoroSettings.longBreakMin,
      pomodorosUntilLongBreak: pomodoroSettings.pomodorosUntilLongBreak,
      autoStartWork: pomodoroSettings.autoStartWork,
      autoStartBreak: pomodoroSettings.autoStartBreak,
      title: ctx.title,
      tasks: ctx.tasks,
      workMinOverride: ctx.workMinOverride,
    };
    try {
      const s = await kepler().request<BackendState>("pomodoro.start", { config });
      applyState(s);
    } catch (e) {
      console.error("[pomodoroSession] start failed:", e);
    }
  }

  async function pause(): Promise<void> {
    await ensureInit();
    try {
      const s = await kepler().request<BackendState>("pomodoro.pause");
      applyState(s);
    } catch (e) { console.error("[pomodoroSession] pause failed:", e); }
  }

  async function resume(): Promise<void> {
    await ensureInit();
    try {
      const s = await kepler().request<BackendState>("pomodoro.resume");
      applyState(s);
    } catch (e) { console.error("[pomodoroSession] resume failed:", e); }
  }

  async function skip(): Promise<void> {
    await ensureInit();
    try {
      const s = await kepler().request<BackendState>("pomodoro.skip");
      applyState(s);
    } catch (e) { console.error("[pomodoroSession] skip failed:", e); }
  }

  async function stop(): Promise<void> {
    await ensureInit();
    try {
      const s = await kepler().request<BackendState>("pomodoro.stop");
      applyState(s);
    } catch (e) { console.error("[pomodoroSession] stop failed:", e); }
  }

  // Permission prompt (как в usePomodoro).
  if (
    typeof Notification !== "undefined" &&
    Notification.permission === "default" &&
    pomodoroSettings.systemNotifications
  ) {
    void Notification.requestPermission().catch(() => {});
  }

  // Lazy init на module-import — стартуем subscribe ASAP чтобы не пропустить
  // backend события (например, активная сессия после reload).
  void ensureInit();

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
    dispose,
  };
}

const session = createSessionState();

export function usePomodoroSession() {
  return session;
}
