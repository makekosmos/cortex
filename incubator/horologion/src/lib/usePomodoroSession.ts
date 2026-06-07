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
import type { TimeEntry } from "../types";
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

  // Дневной счётчик завершённых pomodoros (live за сегодня, локальная дата).
  // Считается из ARK через time_entry_obj с source="pomodoro" + completed=true.
  // Hydrate на init, инкрементируется на pomodoro_finished work,
  // ре-hydrate ещё раз чтобы перейти через полночь / учесть ручные edit'ы.
  const todayCompleted = ref(0);

  // Side-effect state — owned by renderer (как раньше в usePomodoro).
  const currentEntryId = ref<string | null>(null);
  const lastContext = ref<PhaseContext | null>(null);

  // Wallclock natural-finish сигнал: backend emit'ит `pomodoro_finished`
  // строго перед `pomodoro_phase_changed`. Здесь поднимаем флаг во время
  // finished-хэндлера и снимаем в phase_changed → closeArkEntry помечает
  // entry completed=true. Любой другой переход (stop, pause) флаг не
  // поднимает → entry остаётся completed=false (как «брошенный»).
  let pendingWorkCompletion = false;

  // Wallclock anchor для local interpolation. `null` ⇒ idle/paused,
  // remainingMs управляется server'ом напрямую.
  let phaseEndsAtMs: number | null = null;
  let interpTimer: ReturnType<typeof setInterval> | null = null;

  let initialised = false;
  let unsubFns: Array<() => void> = [];

  // Serial queue для всех side-effect операций над ARK time_entry
  // (createArkEntry / closeArkEntry). Без неё pause() и phase_changed handler
  // могут одновременно дёргать close → race условие: двойной stopTimer на
  // одном id, либо create нового entry в момент когда предыдущий ещё не
  // закрыт. Очередь гарантирует строгий порядок и единственного владельца
  // currentEntryId в любой момент времени.
  let sideEffectQueue: Promise<void> = Promise.resolve();
  function enqueueSideEffect(work: () => Promise<void>): Promise<void> {
    const next = sideEffectQueue.then(work, work).catch((e) => {
      console.error("[pomodoroSession] side-effect failed:", e);
    });
    sideEffectQueue = next;
    return next;
  }

  // Throttle: push в focus widget только при смене целой секунды
  // (виджет показывает MM:SS, нет смысла спамить IPC 30 раз / sec).
  let lastFocusPushedSec = -1;

  // Track last applied focus-blocking state to avoid spamming
  // `focus.set_active_state` каждый tick — обновляем только при изменении.
  let lastBlockingApplied: { active: boolean; blocklistId: string | null } | null = null;

  async function applyFocusBlocking(): Promise<void> {
    const blocklistId = pomodoroSettings.focusBlocklistId;
    const shouldBlock =
      phase.value === "work" && isRunning.value && !isPaused.value && blocklistId != null;
    const next = { active: shouldBlock, blocklistId: shouldBlock ? blocklistId : null };
    if (
      lastBlockingApplied &&
      lastBlockingApplied.active === next.active &&
      lastBlockingApplied.blocklistId === next.blocklistId
    ) {
      return;
    }
    lastBlockingApplied = next;
    try {
      const k = (window as unknown as { kepler?: { ark?: KeplerArk } }).kepler;
      if (!k?.ark) return;
      if (shouldBlock) {
        await k.ark.request("focus.set_active_state", {
          active: true,
          blocklist_id: blocklistId,
        });
      } else {
        await k.ark.request("focus.set_active_state", { active: false });
      }
    } catch (e) {
      console.warn("[pomodoroSession] focus.set_active_state unavailable:", e);
    }
  }

  function pushFocusWidgetState(force = false): void {
    const api = typeof window !== "undefined" ? window.kepler?.focusWidget : null;
    if (!api?.setState) return;

    // Виджет видим пока session не idle: running, paused, между фазами
    // (Finished пришёл, isRunning=false, phase=Work/ShortBreak/LongBreak
    // ждёт ручного Skip/Resume — auto_start_*=false). Скрываем только на
    // stop() (phase=idle). См. platform/desktop/electron/focus-widget.ts —
    // deriveFocusStateFromBackend применяет ту же логику.
    const active = phase.value !== "idle";
    const remainingSec = Math.ceil(remainingMs.value / 1000);
    if (!force && active && remainingSec === lastFocusPushedSec) return;
    lastFocusPushedSec = active ? remainingSec : -1;

    const mode: "work" | "break" | "stopwatch" = phase.value === "work" ? "work" : "break";
    const ctxTitle = (lastContext.value?.title ?? pomodoroDraft.value.title ?? "").trim();
    const firstTask = lastContext.value?.tasks?.[0] ?? pomodoroDraft.value.tasks?.[0];
    const label = ctxTitle || firstTask?.title || (mode === "work" ? "Фокус" : "Перерыв");

    const blockingActive =
      phase.value === "work" &&
      isRunning.value &&
      !isPaused.value &&
      pomodoroSettings.focusBlocklistId != null;

    // Передаём wallclock-anchor: когда Horologion окно скрыто, Chromium
    // throttle'ит наш setInterval и renderer перестаёт пушить апдейты.
    // Main process использует phaseEndsAtMs чтобы автономно тикать MM:SS
    // каждую секунду пока виджет видим. На pause / idle — null.
    const widgetPhaseEndsAtMs = active ? phaseEndsAtMs : null;

    void api.setState({
      active,
      remainingSec,
      totalSec: Math.ceil(totalMs.value / 1000),
      label,
      mode,
      blockingActive,
      isPaused: isPaused.value,
      phaseEndsAtMs: widgetPhaseEndsAtMs,
    });
  }

  function recomputeFromAnchor(): void {
    if (phaseEndsAtMs == null) return;
    const next = Math.max(0, phaseEndsAtMs - Date.now());
    if (remainingMs.value !== next) remainingMs.value = next;
    pushFocusWidgetState();
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

    // Push в focus widget на каждом state change (start/pause/stop/phase-flip).
    // Force=true чтобы стейт точно дошёл даже если remainingSec совпадает.
    pushFocusWidgetState(true);
    // Declarative focus-blocking state. Idempotent — обновит backend только
    // если состояние реально изменилось.
    void applyFocusBlocking();
  }

  async function createArkEntry(p: PomodoroPhase, ctx: PhaseContext): Promise<string | null> {
    if (p === "work") {
      const firstTask = ctx.tasks[0];
      const entry = await window.horologion.timeEntries.startTimer({
        title: ctx.title || "Помодоро",
        taskId: firstTask?.id ?? null,
        taskTitle: firstTask?.title ?? null,
        source: "pomodoro",
      });
      notifyEntriesChanged();
      return entry.id;
    }
    if (pomodoroSettings.trackBreaksAsRest) {
      const entry = await window.horologion.timeEntries.startTimer({
        title: "Отдых",
        taskId: null,
        taskTitle: null,
        source: "pomodoro_break",
      });
      notifyEntriesChanged();
      return entry.id;
    }
    return null;
  }

  /**
   * Найти ARK time_entry соответствующий активной pomodoro фазе и восстановить
   * `currentEntryId`. Вызывается после reload extension'а в `ensureInit`,
   * когда backend сообщил что session running, но renderer потерял id.
   *
   * Берём самую свежую running entry с подходящим source:
   *   work / pomodoro → "pomodoro"
   *   shortBreak / longBreak → "pomodoro_break" (если trackBreaksAsRest)
   *
   * Если ничего не нашли — currentEntryId остаётся null; следующий close/
   * pause просто будет no-op. Это лучше чем привязать чужой entry.
   */
  async function rehydrateCurrentEntryId(p: PomodoroPhase): Promise<void> {
    try {
      const wantSource: TimeEntry["source"] = p === "work" ? "pomodoro" : "pomodoro_break";
      const running = await window.horologion.timeEntries.listRunning({ source: wantSource });
      // listRunning сортирует по startedAt desc — берём первую (самую свежую).
      const candidate = running[0];
      if (candidate) {
        currentEntryId.value = candidate.id;
        lastContext.value = {
          title: candidate.title || "Помодоро",
          tasks: candidate.taskId
            ? [{ id: candidate.taskId, title: candidate.taskTitle ?? candidate.title ?? "" }]
            : [],
        };
      }
    } catch (e) {
      console.warn("[pomodoroSession] rehydrate currentEntryId failed:", e);
    }
  }

  async function refreshTodayCompleted(): Promise<void> {
    try {
      todayCompleted.value = await window.horologion.timeEntries.countTodayCompletedPomodoros();
    } catch (e) {
      console.warn("[pomodoroSession] countTodayCompletedPomodoros failed:", e);
    }
  }

  async function closeArkEntry(finishedPhase: PomodoroPhase, completed: boolean): Promise<void> {
    const id = currentEntryId.value;
    if (!id) return;

    const liveTitle = pomodoroDraft.value.title;
    const liveTasks = pomodoroDraft.value.tasks.slice();
    const isWork = finishedPhase === "work";
    const effectiveTitle = (isWork ? liveTitle : lastContext.value?.title) || "Помодоро";
    const effectiveTasks = isWork ? liveTasks : (lastContext.value?.tasks ?? []);

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
              // multi-task split — это всегда работа, source наследуется из
              // оригинальной work-фазы. completed раскидывается на все slice'ы:
              // если основная фаза завершилась natural — каждый slice тоже
              // считается завершённым (они представляют одну непрерывную работу).
              source: "pomodoro",
              completed,
            });
          }
        }
      } catch (e) {
        console.error("[pomodoroSession] failed multi-task split:", e);
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
          completed,
        });
      } catch (e) {
        console.error("[pomodoroSession] failed update before stop:", e);
      }
      try {
        await window.horologion.timeEntries.stopTimer(id);
      } catch {
        /* sidecar мог уже закрыть */
      }
    } else {
      try {
        await window.horologion.timeEntries.stopTimer(id);
      } catch {
        /* sidecar мог уже закрыть */
      }
    }

    currentEntryId.value = null;
    notifyEntriesChanged();
    if (isWork && completed) {
      todayCompleted.value += 1;
    }
  }

  function notifyEnd(p: PomodoroPhase) {
    if (p === "work") playSound(pomodoroSettings.workEndSound);
    else if (p === "shortBreak" || p === "longBreak") playSound(pomodoroSettings.breakEndSound);
    // System Notification вынесен в main process (platform/desktop/electron/pomodoro-notifier.ts).
    // Это даёт toast независимо от того, открыто ли Horologion окно.
  }

  async function ensureInit(): Promise<void> {
    if (initialised) return;
    initialised = true;

    try {
      const s = await kepler().request<BackendState>("pomodoro.get_state");
      applyState(s);
      // Восстановление currentEntryId после reload extension'а. Backend
      // знает что sessions running, но id ARK time_entry живёт в renderer'е.
      // Без re-hydration после reload pause/resume не закроет entry — она
      // повиснет «running вечно» в ARK.
      if (s.isRunning && s.phase !== "idle") {
        await rehydrateCurrentEntryId(s.phase);
      }
    } catch (e) {
      console.error("[pomodoroSession] get_state failed:", e);
    }

    void refreshTodayCompleted();

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

        // Captures флаг до async work — finished event приходит строго перед
        // phase_changed, успевает выставить pendingWorkCompletion=true.
        // Для не-work фаз completed не имеет смысла (всегда false).
        const wasNaturalWorkFinish = from === "work" && pendingWorkCompletion;
        pendingWorkCompletion = false;

        // Серилизуем через sideEffectQueue: pause / phase_changed / resume
        // могут гоняться. Без очереди два close одновременно дадут двойной
        // stopTimer на одном id, либо create нового entry в момент когда
        // предыдущий не закрыт.
        enqueueSideEffect(async () => {
          if (from === "work" || (from !== "idle" && pomodoroSettings.trackBreaksAsRest)) {
            await closeArkEntry(from, wasNaturalWorkFinish);
          }
          if (to === "idle") {
            currentEntryId.value = null;
            return;
          }
          // Защита от двойного create: если из-за быстрого start/restart два
          // phase_changed пришли подряд и для текущей фазы уже есть entry —
          // не дублировать. currentEntryId сбрасывается в closeArkEntry выше,
          // так что если он не null здесь — значит entry актуален для `to`.
          if (s.isRunning && currentEntryId.value == null) {
            const ctx: PhaseContext = {
              title: s.title || lastContext.value?.title || "Помодоро",
              tasks: s.tasks ?? lastContext.value?.tasks ?? [],
            };
            lastContext.value = ctx;
            currentEntryId.value = await createArkEntry(to, ctx);
          }
        });
      }),
    );
    unsubFns.push(
      kepler().subscribe("pomodoro_finished", (payload) => {
        const s = payload as BackendState & { finished?: PomodoroPhase };
        applyState(s);
        // Поднимаем флаг ровно для work natural-finish. Следующий
        // phase_changed (work → break) консьюмит его и пометит entry
        // как completed=true.
        if (s.finished === "work") pendingWorkCompletion = true;
        if (s.finished) notifyEnd(s.finished);
      }),
    );
  }

  function dispose(): void {
    for (const u of unsubFns) {
      try {
        u();
      } catch {
        /* ignore */
      }
    }
    unsubFns = [];
    clearInterpTimer();
    phaseEndsAtMs = null;
    initialised = false;
    // Полный reset state — иначе singleton (см. module-level `session`)
    // протаскивает stale данные между тестами и пользователю при reload:
    // currentEntryId, lastContext, pendingWorkCompletion остаются от предыдущей
    // session. Очередь side effects — сбрасываем на свежий Promise.resolve().
    phase.value = "idle";
    remainingMs.value = 0;
    totalMs.value = 0;
    completedPomodoros.value = 0;
    todayCompleted.value = 0;
    isRunning.value = false;
    isPaused.value = false;
    currentEntryId.value = null;
    lastContext.value = null;
    pendingWorkCompletion = false;
    lastFocusPushedSec = -1;
    lastBlockingApplied = null;
    sideEffectQueue = Promise.resolve();
  }

  async function start(ctx: PhaseContext): Promise<void> {
    await ensureInit();
    // Глубокая копия с принудительной типизацией к примитивам — IPC сериализатор
    // (Electron v8 / structured clone) падает с "An object could not be cloned"
    // на Vue reactive Proxy / Symbol / функциях. `ctx.tasks` приходит как
    // slice() реактивного массива: элементы остаются Proxy. Plain {id,title}
    // снимает обёртку; Number/Boolean — снимают reactive getter'ы pomodoroSettings.
    const tasksPlain = (ctx.tasks ?? []).map((t) => ({
      id: String(t.id),
      title: String(t.title),
    }));
    lastContext.value = {
      title: String(ctx.title ?? ""),
      tasks: tasksPlain,
      workMinOverride: typeof ctx.workMinOverride === "number" ? ctx.workMinOverride : undefined,
    };
    const config = {
      workMin: Number(pomodoroSettings.workMin),
      shortBreakMin: Number(pomodoroSettings.shortBreakMin),
      longBreakMin: Number(pomodoroSettings.longBreakMin),
      pomodorosUntilLongBreak: Number(pomodoroSettings.pomodorosUntilLongBreak),
      autoStartWork: Boolean(pomodoroSettings.autoStartWork),
      autoStartBreak: Boolean(pomodoroSettings.autoStartBreak),
      title: String(ctx.title ?? ""),
      tasks: tasksPlain,
      workMinOverride: typeof ctx.workMinOverride === "number" ? ctx.workMinOverride : undefined,
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
      const phaseAtPause = phase.value;
      const s = await kepler().request<BackendState>("pomodoro.pause");
      applyState(s);
      // Toggl Track-style: pause закрывает текущий time_entry с
      // endedAt = моментом паузы. Иначе backend замораживает remainingMs,
      // но ARK entry остаётся «running» и при последующем stopTimer
      // вбирает в себя всю паузу — итоговая длительность раздувается.
      // pause никогда не считается natural-finish'ем pomodoro — completed=false.
      // Серилизуем через sideEffectQueue чтобы pause не гонялся с
      // phase_changed handler'ом (см. enqueueSideEffect в ensureInit).
      await enqueueSideEffect(() => closeArkEntry(phaseAtPause, false));
    } catch (e) {
      console.error("[pomodoroSession] pause failed:", e);
    }
  }

  async function resume(): Promise<void> {
    await ensureInit();
    try {
      const s = await kepler().request<BackendState>("pomodoro.resume");
      applyState(s);
      // На resume открываем новый сегмент time_entry — startedAt = now.
      // Сумма сегментов = реально отработанное время без пауз.
      if (s.isRunning && !s.isPaused && s.phase !== "idle") {
        await enqueueSideEffect(async () => {
          if (currentEntryId.value != null) return;
          const ctx: PhaseContext = {
            title: s.title || lastContext.value?.title || "Помодоро",
            tasks: s.tasks ?? lastContext.value?.tasks ?? [],
          };
          lastContext.value = ctx;
          currentEntryId.value = await createArkEntry(s.phase, ctx);
        });
      }
    } catch (e) {
      console.error("[pomodoroSession] resume failed:", e);
    }
  }

  async function skip(): Promise<void> {
    await ensureInit();
    try {
      const s = await kepler().request<BackendState>("pomodoro.skip");
      applyState(s);
    } catch (e) {
      console.error("[pomodoroSession] skip failed:", e);
    }
  }

  async function stop(): Promise<void> {
    await ensureInit();
    try {
      const s = await kepler().request<BackendState>("pomodoro.stop");
      applyState(s);
    } catch (e) {
      console.error("[pomodoroSession] stop failed:", e);
    }
  }

  // Permission prompt не нужен: native toast'ы шлёт main process через
  // Electron Notification API (не requires renderer permission grant).

  // Lazy init на module-import — стартуем subscribe ASAP чтобы не пропустить
  // backend события (например, активная сессия после reload).
  void ensureInit();

  // Test-only helper для deterministic дрейна serial очереди. В production коде
  // не нужен — Vue реактивность и собственные awaits хватает. В тестах
  // `await drainSideEffects()` заменяет хрупкие `for (...) await Promise.resolve()`.
  async function drainSideEffects(): Promise<void> {
    await sideEffectQueue;
  }

  return {
    phase,
    remainingMs,
    totalMs,
    completedPomodoros,
    todayCompleted,
    isRunning,
    isPaused,
    progress: computed(() => (totalMs.value > 0 ? 1 - remainingMs.value / totalMs.value : 0)),
    start,
    pause,
    resume,
    skip,
    stop,
    dispose,
    drainSideEffects,
  };
}

const session = createSessionState();

export function usePomodoroSession() {
  return session;
}
