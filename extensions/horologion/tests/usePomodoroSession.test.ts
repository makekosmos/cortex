// Тесты для `usePomodoroSession` — thin wrapper над backend pomodoro
// session. Главное: renderer интерполирует remainingMs локально по
// `phaseEndsAtMs` anchor'у; backend тикает редко (1 Hz), UI плавный.
//
// Backend здесь — fake: мокаем `window.kepler.ark.request` (RPC) и
// `subscribe` (event channel). Тесты драйверят это вручную, проверяя
// что:
//   * Получив phaseEndsAtMs в snapshot/event → remainingMs локально
//     убывает по wallclock.
//   * Pause-event (phaseEndsAtMs=null) → remainingMs замораживается.
//   * Resume-event (новый anchor) → продолжаем интерполяцию от нового anchor'а.

import { afterEach, beforeAll, beforeEach, describe, expect, test } from "bun:test";

// --- Globals/mocks ДО import'а тестируемого модуля ---

// localStorage stub.
const lsStore: Record<string, string> = {};
(globalThis as any).localStorage = {
  getItem: (k: string) => lsStore[k] ?? null,
  setItem: (k: string, v: string) => { lsStore[k] = v; },
  removeItem: (k: string) => { delete lsStore[k]; },
};

// Controlled Date.now.
let currentNow = 0;
const realDateCtor = Date;
class MockedDate extends realDateCtor {
  constructor(...args: any[]) {
    if (args.length === 0) super(currentNow);
    // @ts-expect-error variadic
    else super(...args);
  }
  static now() { return currentNow; }
}
(globalThis as any).Date = MockedDate;

// Controlled setInterval/clearInterval — captures usePomodoroSession's
// interpolation callback so tests могут drive его манально.
let interpCb: (() => void) | null = null;
let interpHandle = 0;
(globalThis as any).setInterval = ((fn: () => void, _ms: number) => {
  interpCb = fn;
  return ++interpHandle as unknown as ReturnType<typeof setInterval>;
}) as typeof setInterval;
(globalThis as any).clearInterval = ((_h: unknown) => {
  interpCb = null;
}) as typeof clearInterval;

// Tick advance helper — двигает clock и invoke'ит interp callback
// (как настоящий 33ms interval).
function advanceTime(ms: number): void {
  // Step ~33ms.
  let remaining = ms;
  while (remaining > 0) {
    const step = Math.min(33, remaining);
    currentNow += step;
    remaining -= step;
    if (interpCb) interpCb();
  }
}

// AudioContext stub (sounds.ts lazy reads).
(globalThis as any).AudioContext = class {
  state = "running"; currentTime = 0; destination = {};
  createOscillator() { return { type: "sine", frequency: { value: 0 }, connect() {}, start() {}, stop() {} }; }
  createGain() { return { gain: { value: 0, setValueAtTime() {}, linearRampToValueAtTime() {} }, connect() {} }; }
  resume() {}
};

// Notification — no-op (отсутствие permission → notifyEnd no-op).
(globalThis as any).Notification = undefined;

// kepler.ark stub.
type RpcHandler = (params?: Record<string, unknown>) => Promise<unknown> | unknown;
const rpcHandlers: Record<string, RpcHandler> = {};
const subscribers: Record<string, Array<(p: unknown) => void>> = {};
function emit(event: string, payload: unknown): void {
  const list = subscribers[event] ?? [];
  for (const h of list) h(payload);
}

// horologion.timeEntries — minimal fake (renderer side-effects ходят
// сюда; нам важно только, чтобы не падало).
type Entry = {
  id: string;
  title: string;
  startedAt: string;
  endedAt: string | null;
  taskId: string | null;
  taskTitle: string | null;
  source?: string;
  completed?: boolean;
};
const entries: Entry[] = [];
let nextEntryId = 1;
const horoApi = {
  timeEntries: {
    list: async () => entries.slice(),
    listRunning: async (opts?: { source?: string | string[] }) => {
      const running = entries.filter((e) => !e.endedAt);
      if (!opts?.source) return running;
      const allowed = new Set(
        Array.isArray(opts.source) ? opts.source : [opts.source],
      );
      return running.filter((e) => allowed.has(e.source ?? "manual"));
    },
    startTimer: async (input: any) => {
      const e: Entry = {
        id: `te-${nextEntryId++}`, title: input.title,
        startedAt: new Date(currentNow).toISOString(), endedAt: null,
        taskId: input.taskId ?? null, taskTitle: input.taskTitle ?? null,
        source: input.source ?? "manual",
        completed: false,
      };
      entries.push(e);
      return e;
    },
    stopTimer: async (id: string) => {
      const e = entries.find((x) => x.id === id);
      if (e && !e.endedAt) e.endedAt = new Date(currentNow).toISOString();
      return e!;
    },
    update: async (input: any) => {
      const e = entries.find((x) => x.id === input.id);
      if (!e) throw new Error("not found");
      if (input.title !== undefined) e.title = input.title;
      if (input.taskId !== undefined) e.taskId = input.taskId ?? null;
      if (input.taskTitle !== undefined) e.taskTitle = input.taskTitle ?? null;
      if (input.completed !== undefined) e.completed = input.completed;
      return e;
    },
    create: async (input: any) => {
      const e: Entry = {
        id: `te-${nextEntryId++}`, ...input,
      };
      entries.push(e);
      return e;
    },
    delete: async (id: string) => {
      const idx = entries.findIndex((x) => x.id === id);
      if (idx >= 0) entries.splice(idx, 1);
    },
    countTodayCompletedPomodoros: async () => {
      return entries.filter(
        (e) => e.source === "pomodoro" && e.completed === true,
      ).length;
    },
  },
  tasks: { list: async () => [] },
  tags: { list: async () => [] },
};

(globalThis as any).window = globalThis;
(globalThis as any).horologion = horoApi;
(globalThis as any).kepler = {
  ark: {
    request: async (operation: string, params?: Record<string, unknown>) => {
      const h = rpcHandlers[operation];
      if (!h) throw new Error(`no mock for ${operation}`);
      return h(params);
    },
    subscribe: (event: string, handler: (p: unknown) => void) => {
      (subscribers[event] ??= []).push(handler);
      return () => {
        const arr = subscribers[event];
        if (!arr) return;
        const i = arr.indexOf(handler);
        if (i >= 0) arr.splice(i, 1);
      };
    },
  },
};

// Default get_state — idle. Нужен до import'а, потому что
// usePomodoroSession делает `void ensureInit()` на module load.
rpcHandlers["pomodoro.get_state"] = async () => ({
  phase: "idle", remainingMs: 0, totalMs: 0,
  completedPomodoros: 0, isRunning: false, isPaused: false,
  title: "", tasks: [], phaseEndsAtMs: null,
});
// applyFocusBlocking вызывается в applyState на module-load ensureInit'е —
// тоже до beforeEach. No-op handler чтобы не было warn'а.
rpcHandlers["focus.set_active_state"] = async () => ({ ok: true });

// --- import после моков ---
const { usePomodoroSession } = await import("../src/lib/usePomodoroSession");

beforeAll(() => {
  currentNow = new Date("2026-01-01T00:00:00.000Z").getTime();
});

beforeEach(() => {
  entries.length = 0;
  nextEntryId = 1;
  // Reset rpc handlers.
  for (const k of Object.keys(rpcHandlers)) delete rpcHandlers[k];
  // Default get_state → idle.
  rpcHandlers["pomodoro.get_state"] = async () => ({
    phase: "idle", remainingMs: 0, totalMs: 0,
    completedPomodoros: 0, isRunning: false, isPaused: false,
    title: "", tasks: [], phaseEndsAtMs: null,
  });
  // Default focus.set_active_state — no-op. usePomodoroSession вызывает его
  // в applyState (declarative focus-blocking); индивидуальные тесты могут
  // override'нуть, если хотят проверить параметры.
  rpcHandlers["focus.set_active_state"] = async () => ({ ok: true });
  // Singleton session переживает между тестами — без явного reset stale
  // currentEntryId / lastContext / sideEffectQueue из предыдущего теста
  // протекают сюда. dispose() выполняет полный reset (см. impl).
  usePomodoroSession().dispose();
});

afterEach(() => {
  interpCb = null;
});

describe("usePomodoroSession local interpolation", () => {
  test("phaseEndsAtMs anchor → remainingMs убывает локально без backend tick'ов", async () => {
    const startNow = currentNow;
    const totalMs = 25 * 60 * 1000;
    const endsAt = startNow + totalMs;

    rpcHandlers["pomodoro.start"] = async () => ({
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: endsAt,
    });

    const p = usePomodoroSession();
    await p.start({ title: "Focus", tasks: [] });

    // Сразу после start: remainingMs ≈ totalMs.
    expect(p.remainingMs.value).toBe(totalMs);
    expect(p.isRunning.value).toBe(true);

    // Двигаем wallclock на 1s БЕЗ backend tick'а — local interp должна
    // обновить remainingMs.
    advanceTime(1000);
    expect(p.remainingMs.value).toBeLessThanOrEqual(totalMs - 1000 + 50);
    expect(p.remainingMs.value).toBeGreaterThanOrEqual(totalMs - 1000 - 50);

    // Ещё 5 сек — линейно.
    advanceTime(5000);
    expect(p.remainingMs.value).toBeLessThanOrEqual(totalMs - 6000 + 50);
    expect(p.remainingMs.value).toBeGreaterThanOrEqual(totalMs - 6000 - 50);
  });

  test("pause event (phaseEndsAtMs=null) → remainingMs замораживается", async () => {
    const totalMs = 25 * 60 * 1000;
    const startNow = currentNow;
    rpcHandlers["pomodoro.start"] = async () => ({
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });

    const p = usePomodoroSession();
    await p.start({ title: "Focus", tasks: [] });
    advanceTime(2000);
    const beforePause = p.remainingMs.value;

    // Backend emit'ит phase_changed → paused state с явным null anchor.
    rpcHandlers["pomodoro.pause"] = async () => ({
      phase: "work", remainingMs: beforePause, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: true,
      title: "", tasks: [], phaseEndsAtMs: null,
    });
    await p.pause();

    // После pause remainingMs не должен меняться, даже если время идёт.
    const frozen = p.remainingMs.value;
    advanceTime(3000);
    expect(p.remainingMs.value).toBe(frozen);
  });

  test("resume event с новым anchor'ом → интерполяция продолжается", async () => {
    const totalMs = 25 * 60 * 1000;
    const startNow = currentNow;
    rpcHandlers["pomodoro.start"] = async () => ({
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });
    const p = usePomodoroSession();
    await p.start({ title: "Focus", tasks: [] });
    advanceTime(2000);
    const remainingAtPause = p.remainingMs.value;

    rpcHandlers["pomodoro.pause"] = async () => ({
      phase: "work", remainingMs: remainingAtPause, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: true,
      title: "", tasks: [], phaseEndsAtMs: null,
    });
    await p.pause();
    advanceTime(5000); // 5s в паузе

    // Resume: backend пересчитывает anchor = now + remainingAtPause.
    rpcHandlers["pomodoro.resume"] = async () => ({
      phase: "work", remainingMs: remainingAtPause, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "", tasks: [], phaseEndsAtMs: currentNow + remainingAtPause,
    });
    await p.resume();

    // Сразу после resume: ≈ remainingAtPause.
    expect(p.remainingMs.value).toBeLessThanOrEqual(remainingAtPause + 50);
    expect(p.remainingMs.value).toBeGreaterThanOrEqual(remainingAtPause - 50);

    // Двигаем 1s → ещё на 1s меньше.
    advanceTime(1000);
    expect(p.remainingMs.value).toBeLessThanOrEqual(remainingAtPause - 1000 + 50);
    expect(p.remainingMs.value).toBeGreaterThanOrEqual(remainingAtPause - 1000 - 50);
  });

  test("backend tick events применяют новый anchor (clock drift correction)", async () => {
    const totalMs = 25 * 60 * 1000;
    const startNow = currentNow;
    rpcHandlers["pomodoro.start"] = async () => ({
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });
    const p = usePomodoroSession();
    await p.start({ title: "Focus", tasks: [] });

    advanceTime(500);
    // Backend tick — emit обновлённый state. Anchor неизменился; remainingMs
    // не должен «прыгнуть» назад если local clock уже двинулся вперёд.
    emit("pomodoro_tick", {
      event: "pomodoro_tick",
      phase: "work", remainingMs: totalMs - 500, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });
    expect(p.remainingMs.value).toBeLessThanOrEqual(totalMs - 500 + 50);
    expect(p.remainingMs.value).toBeGreaterThanOrEqual(totalMs - 500 - 50);
  });

  // Регресс-тест: до фикса pause замораживал backend remainingMs, но time_entry
  // в ARK оставался открытым → последующий stopTimer писал endedAt = now и
  // entry «съедал» всю длительность паузы. Toggl-style решение: pause закрывает
  // segment с endedAt=пауза, resume открывает новый. Сумма сегментов = реальное
  // отработанное время без пауз.
  test("pause закрывает time_entry, resume открывает новый сегмент — пауза не учитывается", async () => {
    const totalMs = 25 * 60 * 1000;
    const startNow = currentNow;
    rpcHandlers["pomodoro.start"] = async () => ({
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });
    rpcHandlers["focus.set_active_state"] = async () => ({ ok: true });

    const p = usePomodoroSession();
    await p.start({ title: "Focus", tasks: [] });

    // Реальный backend на start шлёт pomodoro_phase_changed { from: idle, to: work }
    // — именно в этом handler'е renderer создаёт первый time_entry.
    emit("pomodoro_phase_changed", {
      event: "pomodoro_phase_changed",
      from: "idle", to: "work",
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });
    // Async chain в phase_changed: closeArkEntry → createArkEntry.
    await usePomodoroSession().drainSideEffects();
    expect(entries.length).toBe(1);
    expect(entries[0]!.endedAt).toBeNull();
    const firstStartMs = new Date(entries[0]!.startedAt).getTime();

    // Прошло 5 минут работы.
    advanceTime(5 * 60_000);

    // Pause: backend замораживает, renderer должен закрыть entry.
    rpcHandlers["pomodoro.pause"] = async () => ({
      phase: "work", remainingMs: 20 * 60_000, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: true,
      title: "Focus", tasks: [], phaseEndsAtMs: null,
    });
    await p.pause();

    const running = entries.filter((e) => !e.endedAt);
    expect(running.length).toBe(0);
    const firstSegment = entries[0]!;
    expect(firstSegment.endedAt).not.toBeNull();
    const firstEndMs = new Date(firstSegment.endedAt!).getTime();
    expect(firstEndMs - firstStartMs).toBe(5 * 60_000);

    // Час в паузе.
    advanceTime(60 * 60_000);

    // Resume: новый сегмент стартует.
    rpcHandlers["pomodoro.resume"] = async () => ({
      phase: "work", remainingMs: 20 * 60_000, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: currentNow + 20 * 60_000,
    });
    await p.resume();

    expect(entries.length).toBe(2);
    const secondSegment = entries[1]!;
    expect(secondSegment.endedAt).toBeNull();
    expect(new Date(secondSegment.startedAt).getTime()).toBe(firstEndMs + 60 * 60_000);

    // Дорабатываем 20 минут, work кончился → phase_changed закрывает entry.
    advanceTime(20 * 60_000);
    emit("pomodoro_phase_changed", {
      event: "pomodoro_phase_changed",
      from: "work", to: "shortBreak",
      phase: "shortBreak", remainingMs: 5 * 60_000, totalMs: 5 * 60_000,
      completedPomodoros: 1, isRunning: false, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: null,
    });
    await usePomodoroSession().drainSideEffects();

    // Сумма длительностей segment'ов = 25 минут чистой работы, а не 1ч 25мин.
    const total = entries.reduce((acc, e) => {
      if (!e.endedAt) return acc;
      return acc + (new Date(e.endedAt).getTime() - new Date(e.startedAt).getTime());
    }, 0);
    expect(total).toBe(25 * 60_000);
  });
});

describe("usePomodoroSession daily counter", () => {
  // Helper: drive «start → natural finish → break» цикл и убедиться, что
  // last work entry помечен completed=true и todayCompleted инкрементнулся.
  test("natural work finish помечает entry как completed=true и инкрементит todayCompleted", async () => {
    const totalMs = 25 * 60 * 1000;
    const startNow = currentNow;
    rpcHandlers["pomodoro.start"] = async () => ({
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });
    rpcHandlers["focus.set_active_state"] = async () => ({ ok: true });

    const p = usePomodoroSession();
    const todayBefore = p.todayCompleted.value;
    await p.start({ title: "Focus", tasks: [] });

    emit("pomodoro_phase_changed", {
      event: "pomodoro_phase_changed",
      from: "idle", to: "work",
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });
    await usePomodoroSession().drainSideEffects();
    expect(entries.length).toBe(1);
    expect(entries[0]!.source).toBe("pomodoro");
    expect(entries[0]!.completed).toBe(false);

    advanceTime(totalMs);

    // Natural finish: backend emit'ит pomodoro_finished СТРОГО до phase_changed.
    emit("pomodoro_finished", {
      event: "pomodoro_finished",
      finished: "work",
      phase: "work", remainingMs: 0, totalMs,
      completedPomodoros: 1, isRunning: false, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: null,
    });
    emit("pomodoro_phase_changed", {
      event: "pomodoro_phase_changed",
      from: "work", to: "shortBreak",
      phase: "shortBreak", remainingMs: 5 * 60_000, totalMs: 5 * 60_000,
      completedPomodoros: 1, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: currentNow + 5 * 60_000,
    });
    await usePomodoroSession().drainSideEffects();

    const workEntry = entries.find((e) => e.source === "pomodoro");
    expect(workEntry).toBeDefined();
    expect(workEntry!.completed).toBe(true);
    expect(workEntry!.endedAt).not.toBeNull();
    expect(p.todayCompleted.value).toBe(todayBefore + 1);
  });

  test("manual stop work-фазы оставляет entry completed=false и НЕ инкрементит todayCompleted", async () => {
    const totalMs = 25 * 60 * 1000;
    const startNow = currentNow;
    rpcHandlers["pomodoro.start"] = async () => ({
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });
    rpcHandlers["focus.set_active_state"] = async () => ({ ok: true });
    rpcHandlers["pomodoro.stop"] = async () => ({
      phase: "idle", remainingMs: 0, totalMs: 0,
      completedPomodoros: 0, isRunning: false, isPaused: false,
      title: "", tasks: [], phaseEndsAtMs: null,
    });

    const p = usePomodoroSession();
    const todayBefore = p.todayCompleted.value;
    await p.start({ title: "Focus", tasks: [] });
    emit("pomodoro_phase_changed", {
      event: "pomodoro_phase_changed",
      from: "idle", to: "work",
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });
    await usePomodoroSession().drainSideEffects();

    advanceTime(3 * 60_000);

    // Stop mid-work — никакого pomodoro_finished, только phase_changed → idle.
    await p.stop();
    emit("pomodoro_phase_changed", {
      event: "pomodoro_phase_changed",
      from: "work", to: "idle",
      phase: "idle", remainingMs: 0, totalMs: 0,
      completedPomodoros: 0, isRunning: false, isPaused: false,
      title: "", tasks: [], phaseEndsAtMs: null,
    });
    await usePomodoroSession().drainSideEffects();

    const workEntry = entries.find((e) => e.source === "pomodoro");
    expect(workEntry).toBeDefined();
    expect(workEntry!.completed).toBe(false);
    expect(p.todayCompleted.value).toBe(todayBefore);
  });
});

describe("usePomodoroSession reload rehydration", () => {
  // Регресс-тест для 2026-05-18 audit: до фикса currentEntryId жил только в
  // renderer'е. После reload extension'а (или crash + restart) backend знал что
  // session running, но renderer терял ID open'нутого ARK entry — последующий
  // pause/stop был no-op'ом, entry оставался «running вечно» в ARK.
  // Фикс: ensureInit() ищет running entry по source (pomodoro / pomodoro_break)
  // и восстанавливает currentEntryId. Проверяем через side effect: после
  // rehydrate pause() корректно закрывает существующий entry.
  test("после reload с активной work-фазой pause закрывает существующий ARK entry", async () => {
    const totalMs = 25 * 60 * 1000;

    // Сценарий: до reload в БД уже есть pomodoro entry (создан до crash).
    entries.push({
      id: "te-pre-reload",
      title: "Focus",
      startedAt: new Date(currentNow - 5 * 60_000).toISOString(),
      endedAt: null,
      taskId: null,
      taskTitle: null,
      source: "pomodoro",
      completed: false,
    });

    // Backend сообщает что session всё ещё running.
    rpcHandlers["pomodoro.get_state"] = async () => ({
      phase: "work",
      remainingMs: totalMs - 5 * 60_000,
      totalMs,
      completedPomodoros: 0,
      isRunning: true,
      isPaused: false,
      title: "Focus",
      tasks: [],
      phaseEndsAtMs: currentNow + (totalMs - 5 * 60_000),
    });
    rpcHandlers["pomodoro.pause"] = async () => ({
      phase: "work",
      remainingMs: totalMs - 5 * 60_000,
      totalMs,
      completedPomodoros: 0,
      isRunning: true,
      isPaused: true,
      title: "Focus",
      tasks: [],
      phaseEndsAtMs: null,
    });

    // dispose() + повторный вызов триггерит ensureInit заново.
    const p = usePomodoroSession();
    p.dispose();
    await p.pause();

    // Если rehydrate сработал: единственная running entry → теперь endedAt != null.
    // Если бы не сработало: pause() игнорирует closeArkEntry (currentEntryId=null),
    // entry остаётся `endedAt = null` — orphan.
    const reloadedEntry = entries.find((e) => e.id === "te-pre-reload");
    expect(reloadedEntry).toBeDefined();
    expect(reloadedEntry!.endedAt).not.toBeNull();
  });

  test("double phase_changed подряд НЕ дублирует ARK entry (serial queue + защита от двойного create)", async () => {
    const totalMs = 25 * 60 * 1000;
    const startNow = currentNow;
    rpcHandlers["pomodoro.start"] = async () => ({
      phase: "work", remainingMs: totalMs, totalMs,
      completedPomodoros: 0, isRunning: true, isPaused: false,
      title: "Focus", tasks: [], phaseEndsAtMs: startNow + totalMs,
    });

    const p = usePomodoroSession();
    p.dispose();
    await p.start({ title: "Focus", tasks: [] });

    // Два phase_changed подряд (race condition simulation):
    // backend два раза emit'нул idle→work из-за quick двойного клика по «Старт».
    for (let i = 0; i < 2; i++) {
      emit("pomodoro_phase_changed", {
        event: "pomodoro_phase_changed",
        from: "idle", to: "work",
        phase: "work", remainingMs: totalMs, totalMs,
        completedPomodoros: 0, isRunning: true, isPaused: false,
        title: "Focus", tasks: [], phaseEndsAtMs: startNow + totalMs,
      });
    }
    // Дрейним serial queue до завершения всех side-effects.
    await usePomodoroSession().drainSideEffects();

    // Только один pomodoro entry — без serial queue + защиты было бы два.
    const pomos = entries.filter((e) => e.source === "pomodoro" && !e.endedAt);
    expect(pomos.length).toBe(1);
  });
});
