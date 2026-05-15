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
};
const entries: Entry[] = [];
let nextEntryId = 1;
const horoApi = {
  timeEntries: {
    list: async () => entries.slice(),
    listRunning: async () => entries.filter((e) => !e.endedAt),
    startTimer: async (input: any) => {
      const e: Entry = {
        id: `te-${nextEntryId++}`, title: input.title,
        startedAt: new Date(currentNow).toISOString(), endedAt: null,
        taskId: input.taskId ?? null, taskTitle: input.taskTitle ?? null,
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
});
