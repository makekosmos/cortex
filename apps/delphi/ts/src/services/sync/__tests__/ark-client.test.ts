import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { fetchTasksFromArk } from "../ark-client";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a raw event as the server returns in /events */
function makeEvent(
  id: string,
  title: string,
  opts: {
    source?: string | null;
    isCompleted?: boolean;
    isCancelled?: boolean;
    isTrashed?: boolean;
    eventType?: string;
  } = {},
): Record<string, unknown> {
  return {
    id: `evt-${id}`,
    event_type: opts.eventType ?? "task",
    source: opts.source ?? null,
    source_id: id,
    summary: title,
    occurred_at: "2025-01-01T00:00:00Z",
    data: {
      id,
      title,
      isCompleted: opts.isCompleted ?? false,
      isCancelled: opts.isCancelled ?? false,
      isTrashed: opts.isTrashed ?? false,
      isToday: false,
      isEvening: false,
      isSomeday: false,
      priority: 0,
      sortOrder: 0,
      createdAt: "2025-01-01T00:00:00Z",
      tagIds: [],
      checklistItems: [],
    },
  };
}

function mockFetch(events: Record<string, unknown>[]) {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: true,
      json: async () => events,
    }),
  );
}

// ---------------------------------------------------------------------------
// Setup: inject Ark credentials into localStorage before each test
// ---------------------------------------------------------------------------

beforeEach(() => {
  localStorage.setItem("delphi.ark_url", "http://localhost:8000");
  localStorage.setItem("delphi.ark_api_key", "test-key");
});

afterEach(() => {
  localStorage.clear();
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("fetchTasksFromArk — deduplication", () => {
  it("deduplicates events with the same id (case-insensitive) → returns one TodoItem", async () => {
    const lowerUuid = "aaaaaaaa-0000-0000-0000-000000000001";
    const upperUuid = "AAAAAAAA-0000-0000-0000-000000000001";

    // Server returns the same logical task twice: once with lowercase id
    // (e.g. source=null) and once with uppercase id (source=delphi-android)
    mockFetch([
      makeEvent(lowerUuid, "Buy milk", { source: null, isCompleted: false }),
      makeEvent(upperUuid, "Buy milk", {
        source: "delphi-android",
        isCompleted: false,
      }),
    ]);

    const todos = await fetchTasksFromArk();

    expect(todos).toHaveLength(1);
    // The id from the first processed event (lowercase) is kept
    expect(todos[0].id.toLowerCase()).toBe(lowerUuid.toLowerCase());
    expect(todos[0].title).toBe("Buy milk");
  });

  it("when duplicate has isCompleted=true, keeps completed version over incomplete", async () => {
    const id = "bbbbbbbb-0000-0000-0000-000000000002";

    // First event: source=null, isCompleted=false (e.g. task was just created)
    // Second event: source=delphi-android, isCompleted=true (completed on Android)
    mockFetch([
      makeEvent(id, "Call doctor", { source: null, isCompleted: false }),
      makeEvent(id, "Call doctor", {
        source: "delphi-android",
        isCompleted: true,
      }),
    ]);

    const todos = await fetchTasksFromArk();

    expect(todos).toHaveLength(1);
    expect(todos[0].isCompleted).toBe(true);
  });

  it("when completed arrives before incomplete, completed version is retained", async () => {
    const id = "cccccccc-0000-0000-0000-000000000003";

    // Order reversed: completed first, incomplete second
    mockFetch([
      makeEvent(id, "Dentist appointment", {
        source: "delphi",
        isCompleted: true,
      }),
      makeEvent(id, "Dentist appointment", {
        source: null,
        isCompleted: false,
      }),
    ]);

    const todos = await fetchTasksFromArk();

    expect(todos).toHaveLength(1);
    expect(todos[0].isCompleted).toBe(true);
  });

  it("unique tasks are all preserved (no false deduplication)", async () => {
    mockFetch([
      makeEvent("id-1", "Task Alpha", { source: null }),
      makeEvent("id-2", "Task Beta", { source: "delphi" }),
      makeEvent("id-3", "Task Gamma", { source: "delphi-android" }),
    ]);

    const todos = await fetchTasksFromArk();

    expect(todos).toHaveLength(3);
    const titles = todos.map((t) => t.title).toSorted();
    expect(titles).toEqual(["Task Alpha", "Task Beta", "Task Gamma"]);
  });

  it("events with non-task event_type are filtered out", async () => {
    mockFetch([
      makeEvent("id-task", "A real task", { eventType: "task" }),
      makeEvent("id-proj", "A project", { eventType: "project" }),
    ]);

    const todos = await fetchTasksFromArk();

    expect(todos).toHaveLength(1);
    expect(todos[0].title).toBe("A real task");
  });

  it("returns [] when Ark URL is not configured", async () => {
    localStorage.removeItem("delphi.ark_url");
    // fetch should not even be called
    const spy = vi.fn();
    vi.stubGlobal("fetch", spy);

    const todos = await fetchTasksFromArk();

    expect(todos).toEqual([]);
    expect(spy).not.toHaveBeenCalled();
  });

  it("returns [] when server responds with non-ok status", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: false, status: 401 }),
    );

    const todos = await fetchTasksFromArk();

    expect(todos).toEqual([]);
  });

  it("returns [] when fetch throws (network error)", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockRejectedValue(new Error("Network error")),
    );

    const todos = await fetchTasksFromArk();

    expect(todos).toEqual([]);
  });

  it("massive duplication scenario: same id from 4 sources → one task", async () => {
    const id = "dddddddd-0000-0000-0000-000000000004";
    const sources = [null, "delphi", "delphi-android", "delphi-web"];

    mockFetch(
      sources.map((source) =>
        makeEvent(id, "Duplicated task", { source, isCompleted: false }),
      ),
    );

    const todos = await fetchTasksFromArk();

    expect(todos).toHaveLength(1);
    expect(todos[0].title).toBe("Duplicated task");
  });
});
