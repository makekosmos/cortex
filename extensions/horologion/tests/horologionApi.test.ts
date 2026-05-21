// Тесты для `horologionApi` — адаптер ARK операций для Horologion extension.
//
// Здесь фокус на `countTodayCompletedPomodoros` и пробрасывание `source` /
// `completed` через startTimer/createTimeEntry/update. Используется in-memory
// stub для `window.kepler.ark.request` — каждая операция (list_objects_by_type,
// upsert_object, get_object, delete_object) обслуживается ручным handler'ом.

import { beforeEach, describe, expect, test } from "bun:test";

import type { ArkObjectRecord } from "@kosmos/ark";

// ARK store — простая map по id.
const store = new Map<string, ArkObjectRecord>();

function listByType(typeId: string): ArkObjectRecord[] {
  return [...store.values()].filter((o) => o.typeId === typeId);
}

(globalThis as any).window = globalThis;
(globalThis as any).kepler = {
  ark: {
    request: async (operation: string, params?: Record<string, unknown>) => {
      switch (operation) {
        case "list_objects_by_type": {
          const typeId = (params?.type_id as string) ?? "";
          return listByType(typeId);
        }
        case "list_running_time_entries": {
          const sourceFilter = params?.source as string | undefined;
          return listByType("time_entry_obj")
            .filter((o) => !o.deletedAt)
            .filter((o) => {
              const props = (o.propsJson ?? {}) as Record<string, unknown>;
              if (props.endedAt != null && props.endedAt !== "") return false;
              if (sourceFilter && props.source !== sourceFilter) return false;
              return true;
            })
            .sort((a, b) => {
              const ap = ((a.propsJson ?? {}) as Record<string, unknown>).startedAt ?? "";
              const bp = ((b.propsJson ?? {}) as Record<string, unknown>).startedAt ?? "";
              return String(bp).localeCompare(String(ap));
            });
        }
        case "get_object": {
          const id = (params?.id as string) ?? "";
          return store.get(id) ?? null;
        }
        case "upsert_object": {
          const obj = (params as { object: ArkObjectRecord }).object;
          store.set(obj.id, obj);
          return obj;
        }
        case "delete_object": {
          const id = (params?.id as string) ?? "";
          store.delete(id);
          return null;
        }
        default:
          throw new Error(`no mock for ${operation}`);
      }
    },
    subscribe: () => () => {},
  },
};

// --- import после моков ---
const { horologionApi } = await import("../src/lib/horologionApi");

beforeEach(() => {
  store.clear();
});

function makeEntry(args: {
  id: string;
  source?: string;
  completed?: boolean;
  startedAt: string;
  endedAt?: string | null;
  title?: string;
  deletedAt?: string | null;
}): ArkObjectRecord {
  return {
    id: args.id,
    typeId: "time_entry_obj",
    title: args.title ?? "Помодоро",
    contentJson: {},
    propsJson: {
      startedAt: args.startedAt,
      endedAt: args.endedAt ?? null,
      source: args.source ?? "manual",
      billable: false,
      taskId: null,
      taskTitle: null,
      completed: args.completed ?? false,
    },
    createdAt: args.startedAt,
    updatedAt: args.startedAt,
    deletedAt: args.deletedAt ?? null,
  };
}

describe("horologionApi.countTodayCompletedPomodoros", () => {
  test("считает только source='pomodoro' + completed=true за сегодня", async () => {
    const now = new Date();
    const todayMorning = new Date(
      now.getFullYear(),
      now.getMonth(),
      now.getDate(),
      9,
      0,
      0,
    ).toISOString();
    const todayAfternoon = new Date(
      now.getFullYear(),
      now.getMonth(),
      now.getDate(),
      14,
      0,
      0,
    ).toISOString();

    store.set(
      "a",
      makeEntry({ id: "a", source: "pomodoro", completed: true, startedAt: todayMorning }),
    );
    store.set(
      "b",
      makeEntry({ id: "b", source: "pomodoro", completed: true, startedAt: todayAfternoon }),
    );

    const n = await horologionApi.timeEntries.countTodayCompletedPomodoros();
    expect(n).toBe(2);
  });

  test("исключает завершённые pomodoros за вчера (date filter)", async () => {
    const now = new Date();
    const yesterday = new Date(now.getTime() - 24 * 60 * 60 * 1000).toISOString();
    const today = new Date(
      now.getFullYear(),
      now.getMonth(),
      now.getDate(),
      10,
      0,
      0,
    ).toISOString();

    store.set(
      "y",
      makeEntry({ id: "y", source: "pomodoro", completed: true, startedAt: yesterday }),
    );
    store.set("t", makeEntry({ id: "t", source: "pomodoro", completed: true, startedAt: today }));

    expect(await horologionApi.timeEntries.countTodayCompletedPomodoros()).toBe(1);
  });

  test("исключает source='manual' (stopwatch) даже если completed=true", async () => {
    const today = new Date().toISOString();
    store.set("m", makeEntry({ id: "m", source: "manual", completed: true, startedAt: today }));
    store.set("p", makeEntry({ id: "p", source: "pomodoro", completed: true, startedAt: today }));
    expect(await horologionApi.timeEntries.countTodayCompletedPomodoros()).toBe(1);
  });

  test("исключает source='pomodoro_break' (отдых), даже если completed=true", async () => {
    const today = new Date().toISOString();
    store.set(
      "br",
      makeEntry({ id: "br", source: "pomodoro_break", completed: true, startedAt: today }),
    );
    store.set("p", makeEntry({ id: "p", source: "pomodoro", completed: true, startedAt: today }));
    expect(await horologionApi.timeEntries.countTodayCompletedPomodoros()).toBe(1);
  });

  test("исключает completed=false (брошенные/прерванные)", async () => {
    const today = new Date().toISOString();
    store.set("f", makeEntry({ id: "f", source: "pomodoro", completed: false, startedAt: today }));
    store.set("t", makeEntry({ id: "t", source: "pomodoro", completed: true, startedAt: today }));
    expect(await horologionApi.timeEntries.countTodayCompletedPomodoros()).toBe(1);
  });

  test("исключает deleted (soft-deleted) entries", async () => {
    const today = new Date().toISOString();
    store.set(
      "d",
      makeEntry({
        id: "d",
        source: "pomodoro",
        completed: true,
        startedAt: today,
        deletedAt: today,
      }),
    );
    store.set("ok", makeEntry({ id: "ok", source: "pomodoro", completed: true, startedAt: today }));
    expect(await horologionApi.timeEntries.countTodayCompletedPomodoros()).toBe(1);
  });

  test("исключает entries без startedAt", async () => {
    store.set("e", makeEntry({ id: "e", source: "pomodoro", completed: true, startedAt: "" }));
    expect(await horologionApi.timeEntries.countTodayCompletedPomodoros()).toBe(0);
  });

  test("пустой store → 0", async () => {
    expect(await horologionApi.timeEntries.countTodayCompletedPomodoros()).toBe(0);
  });
});

describe("horologionApi.startTimer source/completed propagation", () => {
  test("source default = 'manual', completed = false", async () => {
    const e = await horologionApi.timeEntries.startTimer({ title: "x" });
    expect(e.source).toBe("manual");
    expect(e.completed).toBe(false);
  });

  test("source='pomodoro' прокидывается в propsJson", async () => {
    const e = await horologionApi.timeEntries.startTimer({ title: "x", source: "pomodoro" });
    expect(e.source).toBe("pomodoro");
    // А ещё persistится в store — следующий list увидит source.
    const raw = [...store.values()][0]!;
    expect((raw.propsJson as Record<string, unknown>).source).toBe("pomodoro");
  });

  test("source='pomodoro_break' прокидывается", async () => {
    const e = await horologionApi.timeEntries.startTimer({
      title: "Отдых",
      source: "pomodoro_break",
    });
    expect(e.source).toBe("pomodoro_break");
  });
});

describe("horologionApi.listRunning source filter", () => {
  test("без opts.source возвращает все running entries", async () => {
    const now = new Date().toISOString();
    store.set("p", makeEntry({ id: "p", source: "pomodoro", startedAt: now }));
    store.set("b", makeEntry({ id: "b", source: "pomodoro_break", startedAt: now }));
    store.set("m", makeEntry({ id: "m", source: "manual", startedAt: now }));
    const all = await horologionApi.timeEntries.listRunning();
    expect(all).toHaveLength(3);
  });

  test("opts.source='manual' исключает pomodoro/pomodoro_break (regression: StopwatchView не должна видеть pomodoro_break)", async () => {
    const now = new Date().toISOString();
    store.set("p", makeEntry({ id: "p", source: "pomodoro", startedAt: now }));
    store.set("b", makeEntry({ id: "b", source: "pomodoro_break", startedAt: now }));
    store.set("m", makeEntry({ id: "m", source: "manual", startedAt: now }));
    const manualOnly = await horologionApi.timeEntries.listRunning({ source: "manual" });
    expect(manualOnly).toHaveLength(1);
    expect(manualOnly[0]!.source).toBe("manual");
  });

  test("opts.source принимает массив (для rehydrate pomodoro session)", async () => {
    const now = new Date().toISOString();
    store.set("p", makeEntry({ id: "p", source: "pomodoro", startedAt: now }));
    store.set("b", makeEntry({ id: "b", source: "pomodoro_break", startedAt: now }));
    store.set("m", makeEntry({ id: "m", source: "manual", startedAt: now }));
    const pomos = await horologionApi.timeEntries.listRunning({
      source: ["pomodoro", "pomodoro_break"],
    });
    expect(pomos).toHaveLength(2);
    expect(pomos.map((e) => e.source).sort()).toEqual(["pomodoro", "pomodoro_break"].sort());
  });

  test("orphan entries (без startedAt) фильтруются даже при source=manual", async () => {
    store.set("e", makeEntry({ id: "e", source: "manual", startedAt: "" }));
    const list = await horologionApi.timeEntries.listRunning({ source: "manual" });
    expect(list).toHaveLength(0);
  });
});

describe("horologionApi.update completed propagation", () => {
  test("update({completed:true}) пишет flag в propsJson", async () => {
    const e = await horologionApi.timeEntries.startTimer({ title: "x", source: "pomodoro" });
    expect(e.completed).toBe(false);

    const updated = await horologionApi.timeEntries.update({ id: e.id, completed: true });
    expect(updated.completed).toBe(true);

    // А ещё попадает в counter.
    expect(await horologionApi.timeEntries.countTodayCompletedPomodoros()).toBe(1);
  });

  test("update без completed не сбрасывает существующий флаг", async () => {
    const e = await horologionApi.timeEntries.startTimer({ title: "x", source: "pomodoro" });
    await horologionApi.timeEntries.update({ id: e.id, completed: true });
    // Теперь меняем только title — completed должен остаться true.
    const re = await horologionApi.timeEntries.update({ id: e.id, title: "y" });
    expect(re.completed).toBe(true);
  });
});
