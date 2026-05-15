// Golden tests для usePomodoro — baseline state-machine поведения TS-импла.
// Это контракт, который Rust-port (pomodoro::Session) должен сохранить.
//
// Что мокается:
//   * `window.horologion.timeEntries.*`  — In-memory fake (без ARK).
//   * `window.localStorage`              — minimal stub для pomodoroSettings.
//   * `window.AudioContext`              — stub: playSound становится no-op.
//   * `Notification`                     — отсутствует → notifyEnd no-op.
//   * `Date.now`                         — controllable clock.
//   * `setInterval` / `clearInterval`    — controllable ticker (manual tick()).
//
// Singleton ловушка: `usePomodoro` — module-singleton. Между сценариями
// каждый test вызывает `p.stop()` для reset.

// --- Globals/mocks setup БЕФОР import'а тестируемого модуля. ---

import { afterEach, beforeAll, beforeEach, describe, expect, test } from "bun:test";

// --- localStorage stub ---
const lsStore: Record<string, string> = {};
(globalThis as any).localStorage = {
  getItem: (k: string) => lsStore[k] ?? null,
  setItem: (k: string, v: string) => {
    lsStore[k] = v;
  },
  removeItem: (k: string) => {
    delete lsStore[k];
  },
};

// --- window stub (минимум, поверх globalThis) ---
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
    startTimer: async (input: {
      title: string;
      taskId?: string | null;
      taskTitle?: string | null;
    }) => {
      const e: Entry = {
        id: `te-${nextEntryId++}`,
        title: input.title,
        startedAt: new Date(currentNow).toISOString(),
        endedAt: null,
        taskId: input.taskId ?? null,
        taskTitle: input.taskTitle ?? null,
      };
      entries.push(e);
      return e;
    },
    stopTimer: async (id: string) => {
      const e = entries.find((x) => x.id === id);
      if (e && !e.endedAt) e.endedAt = new Date(currentNow).toISOString();
      return e!;
    },
    update: async (input: {
      id: string;
      title?: string;
      taskId?: string | null;
      taskTitle?: string | null;
    }) => {
      const e = entries.find((x) => x.id === input.id);
      if (!e) throw new Error(`entry ${input.id} not found`);
      if (input.title !== undefined) e.title = input.title;
      if (input.taskId !== undefined) e.taskId = input.taskId ?? null;
      if (input.taskTitle !== undefined) e.taskTitle = input.taskTitle ?? null;
      return e;
    },
    create: async (input: {
      title: string;
      startedAt: string;
      endedAt: string;
      taskId: string | null;
      taskTitle: string | null;
    }) => {
      const e: Entry = {
        id: `te-${nextEntryId++}`,
        title: input.title,
        startedAt: input.startedAt,
        endedAt: input.endedAt,
        taskId: input.taskId,
        taskTitle: input.taskTitle,
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

// AudioContext stub (sounds.ts read'ит `window.AudioContext` lazily).
(globalThis as any).AudioContext = class {
  state = "running";
  currentTime = 0;
  destination = {};
  createOscillator() {
    return {
      type: "sine",
      frequency: { value: 0 },
      connect() {},
      start() {},
      stop() {},
    };
  }
  createGain() {
    return { gain: { value: 0, setValueAtTime() {}, linearRampToValueAtTime() {} }, connect() {} };
  }
  resume() {}
};

// Контролируемые часы.
let currentNow = 0;
const realDateNow = Date.now;
Date.now = () => currentNow;
const realDateCtor = Date;
class MockedDate extends realDateCtor {
  constructor(...args: any[]) {
    if (args.length === 0) {
      super(currentNow);
    } else {
      // @ts-expect-error variadic
      super(...args);
    }
  }
  static now() {
    return currentNow;
  }
}
(globalThis as any).Date = MockedDate;

// Контролируемый ticker: usePomodoro дёргает setInterval(..., 250).
// Сохраняем callback, тестам предоставляем `advance(ms)` который двигает
// часы и вручную дёргает callback по 250ms-«фронтам».
let tickerCb: (() => void) | null = null;
const realSetInterval = globalThis.setInterval;
const realClearInterval = globalThis.clearInterval;
(globalThis as any).setInterval = ((fn: () => void, _ms: number) => {
  tickerCb = fn;
  return 1 as unknown as ReturnType<typeof setInterval>;
}) as typeof setInterval;
(globalThis as any).clearInterval = ((_handle: unknown) => {
  tickerCb = null;
}) as typeof clearInterval;

function advance(ms: number): void {
  // 250ms per "frame" — драйверим callback пошагово, как живой setInterval.
  const step = 250;
  let remaining = ms;
  while (remaining > 0) {
    const delta = Math.min(step, remaining);
    currentNow += delta;
    remaining -= delta;
    if (tickerCb) tickerCb();
  }
}

// --- импортируем модули ПОСЛЕ установки моков ---
const { pomodoroSettings, DEFAULT_POMODORO_SETTINGS } = await import(
  "../src/lib/pomodoroSettings"
);
const { usePomodoro } = await import("../src/lib/usePomodoro");

beforeAll(() => {
  // baseline starting clock — далеко в будущем, чтобы избежать ISO-0 edge.
  currentNow = new Date("2026-01-01T00:00:00.000Z").getTime();
});

beforeEach(async () => {
  // Reset settings → defaults; reset entries.
  Object.assign(pomodoroSettings, DEFAULT_POMODORO_SETTINGS);
  // Чтобы избежать autoStartBreak в base scenarios — выключаем; включается явно где надо.
  pomodoroSettings.autoStartWork = false;
  pomodoroSettings.autoStartBreak = false;
  pomodoroSettings.systemNotifications = false;
  pomodoroSettings.workEndSound = "none";
  pomodoroSettings.breakEndSound = "none";
  entries.length = 0;
  nextEntryId = 1;
  const p = usePomodoro();
  await p.stop();
});

afterEach(() => {
  // ensure ticker callback освобождён между тестами
  tickerCb = null;
});

describe("usePomodoro state machine (golden)", () => {
  test("idle → start → phase=work, isRunning, remainingMs ≈ workMin*60000", async () => {
    const p = usePomodoro();
    expect(p.phase.value).toBe("idle");
    expect(p.isRunning.value).toBe(false);

    await p.start({ title: "Focus", tasks: [] });
    expect(p.phase.value).toBe("work");
    expect(p.isRunning.value).toBe(true);
    expect(p.isPaused.value).toBe(false);
    expect(p.totalMs.value).toBe(pomodoroSettings.workMin * 60 * 1000);
    // remainingMs только что был установлен → equals total.
    expect(p.remainingMs.value).toBe(p.totalMs.value);
  });

  test("tick monotonically decreases remainingMs", async () => {
    const p = usePomodoro();
    await p.start({ title: "Focus", tasks: [] });
    const before = p.remainingMs.value;
    advance(5 * 60 * 1000); // 5 минут
    expect(p.remainingMs.value).toBeLessThan(before);
    expect(p.remainingMs.value).toBeGreaterThan(0);
    expect(p.remainingMs.value).toBe(before - 5 * 60 * 1000);
  });

  test("pause freezes remainingMs; resume continues", async () => {
    const p = usePomodoro();
    await p.start({ title: "Focus", tasks: [] });
    advance(60_000); // 1 минута
    const frozen = p.remainingMs.value;
    p.pause();
    expect(p.isPaused.value).toBe(true);
    advance(30_000);
    expect(p.remainingMs.value).toBe(frozen);
    p.resume();
    expect(p.isPaused.value).toBe(false);
    advance(60_000);
    expect(p.remainingMs.value).toBe(frozen - 60_000);
  });

  test("finish work → next phase = shortBreak (1st pomodoro), completed=1", async () => {
    const p = usePomodoro();
    await p.start({ title: "Focus", tasks: [] });
    // Прогоним до конца work-фазы.
    advance(pomodoroSettings.workMin * 60 * 1000);
    // tick triggers finishPhase которая is async — даём микротаскам сойти.
    await new Promise((r) => setTimeout(r, 0));
    await new Promise((r) => setTimeout(r, 0));
    expect(p.completedPomodoros.value).toBe(1);
    // autoStartBreak=false → phase переключился но не запущен.
    expect(p.phase.value).toBe("shortBreak");
    expect(p.isRunning.value).toBe(false);
  });

  test("4-й work завершившийся → longBreak", async () => {
    pomodoroSettings.autoStartBreak = true;
    pomodoroSettings.autoStartWork = true;
    const p = usePomodoro();
    // Skip()-driven cycle — детерминистично, без зависимости от auto-start ticker'а
    // (auto-start через скип реализован тем же путём что и через timer's finishPhase).
    await p.start({ title: "Focus", tasks: [] });
    for (let i = 0; i < 4; i++) {
      // skip() во время work — finishes immediately, autoStartBreak → break-фаза running
      await p.skip();
      await new Promise((r) => setTimeout(r, 0));
      await new Promise((r) => setTimeout(r, 0));
      if (i < 3) {
        // skip break → autoStartWork → следующий work running
        await p.skip();
        await new Promise((r) => setTimeout(r, 0));
        await new Promise((r) => setTimeout(r, 0));
      }
    }
    expect(p.completedPomodoros.value).toBe(4);
    expect(p.phase.value).toBe("longBreak");
  });

  test("skip() во время work — финиширует немедленно", async () => {
    const p = usePomodoro();
    await p.start({ title: "Focus", tasks: [] });
    advance(60_000);
    await p.skip();
    await new Promise((r) => setTimeout(r, 0));
    expect(p.completedPomodoros.value).toBe(1);
    expect(p.phase.value).toBe("shortBreak");
  });

  test("stop() — phase=idle, completedPomodoros=0", async () => {
    const p = usePomodoro();
    await p.start({ title: "Focus", tasks: [] });
    advance(60_000);
    await p.stop();
    expect(p.phase.value).toBe("idle");
    expect(p.isRunning.value).toBe(false);
    expect(p.completedPomodoros.value).toBe(0);
    expect(p.remainingMs.value).toBe(0);
  });

  test("autoStartBreak=true → после work авто-стартует shortBreak", async () => {
    pomodoroSettings.autoStartBreak = true;
    const p = usePomodoro();
    await p.start({ title: "Focus", tasks: [] });
    advance(pomodoroSettings.workMin * 60 * 1000);
    await new Promise((r) => setTimeout(r, 0));
    await new Promise((r) => setTimeout(r, 0));
    expect(p.phase.value).toBe("shortBreak");
    expect(p.isRunning.value).toBe(true);
    expect(p.totalMs.value).toBe(pomodoroSettings.shortBreakMin * 60 * 1000);
  });

  test("workMinOverride override длительности фокус-сегмента", async () => {
    const p = usePomodoro();
    await p.start({ title: "Focus", tasks: [], workMinOverride: 50 });
    expect(p.totalMs.value).toBe(50 * 60 * 1000);
  });

  test("createArkEntry start: time_entry создан при start(work)", async () => {
    const p = usePomodoro();
    await p.start({ title: "Focus", tasks: [{ id: "t1", title: "Task A" }] });
    expect(entries.length).toBe(1);
    expect(entries[0]!.title).toBe("Focus");
    expect(entries[0]!.taskId).toBe("t1");
    expect(entries[0]!.endedAt).toBeNull();
  });

  test("stop() закрывает open time_entry", async () => {
    const p = usePomodoro();
    await p.start({ title: "Focus", tasks: [] });
    advance(60_000);
    await p.stop();
    expect(entries.length).toBe(1);
    expect(entries[0]!.endedAt).not.toBeNull();
  });
});
