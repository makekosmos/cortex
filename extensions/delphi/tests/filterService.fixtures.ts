// Shared fixtures для TS golden tests + Rust parity check.
//
// Каждый TodoItem — minimal shape для filtering. Не все поля заполнены,
// только те что читают predicates / sorters. Order specific — golden
// outputs ниже зависят от createdAt / sortOrder.
//
// `today` — фиксированная дата 2026-05-15. isDateToday() в TS читает
// new Date() — для теста делать parametric, см. filterService.test.ts.

import type { TodoItem } from "../src/types/task";

export const TODAY_ISO = "2026-05-15";

// Helper для краткости.
function mk(
  id: string,
  overrides: Partial<TodoItem> = {},
): TodoItem {
  return {
    id,
    title: id,
    notes: null,
    priority: 0,
    scheduledDate: null,
    deadline: null,
    reminderDate: null,
    isToday: false,
    isEvening: false,
    isSomeday: false,
    isCompleted: false,
    completedAt: null,
    isCancelled: false,
    cancelledAt: null,
    isTrashed: false,
    sortOrder: 0,
    createdAt: "2026-05-15T10:00:00.000Z",
    headingId: null,
    projectId: null,
    areaId: null,
    tagIds: [],
    checklistItems: [],
    recurrenceRule: null,
    billable: false,
    price: null,
    ...overrides,
  };
}

export const FIXTURES: TodoItem[] = [
  // Inbox: no project, not someday, active.
  mk("inbox-1", { sortOrder: 1, createdAt: "2026-05-13T10:00:00.000Z" }),
  mk("inbox-2", { sortOrder: 2, createdAt: "2026-05-14T10:00:00.000Z" }),

  // Today: isToday=true.
  mk("today-flag", { isToday: true, sortOrder: 10 }),

  // Today: scheduledDate today (parametric — see test file).
  mk("today-scheduled", { scheduledDate: TODAY_ISO, sortOrder: 5 }),

  // Upcoming: scheduledDate future.
  mk("upcoming-1", {
    scheduledDate: "2026-06-01",
    sortOrder: 100,
    createdAt: "2026-05-10T00:00:00.000Z",
  }),
  mk("upcoming-2", {
    scheduledDate: "2026-05-20",
    sortOrder: 99,
    createdAt: "2026-05-11T00:00:00.000Z",
  }),

  // Someday: isSomeday=true.
  mk("someday-1", { isSomeday: true, sortOrder: 50 }),
  mk("someday-2", { isSomeday: true, sortOrder: 51 }),

  // Project task: has projectId — НЕ Inbox.
  mk("proj-task", {
    projectId: "proj-1",
    sortOrder: 7,
    scheduledDate: "2026-05-20",
  }),

  // Logbook: completed.
  mk("completed-1", {
    isCompleted: true,
    completedAt: "2026-05-14T12:00:00.000Z",
  }),
  mk("completed-2", {
    isCompleted: true,
    completedAt: "2026-05-15T08:00:00.000Z",
  }),

  // Logbook: cancelled.
  mk("cancelled-1", {
    isCancelled: true,
    cancelledAt: "2026-05-13T15:00:00.000Z",
  }),

  // Trash: isTrashed.
  mk("trash-1", { isTrashed: true, createdAt: "2026-05-12T10:00:00.000Z" }),
  mk("trash-2", { isTrashed: true, createdAt: "2026-05-15T11:00:00.000Z" }),

  // Edge: scheduled today + completed (Logbook, не Today).
  mk("scheduled-today-completed", {
    scheduledDate: TODAY_ISO,
    isCompleted: true,
    completedAt: "2026-05-15T10:00:00.000Z",
  }),

  // Edge: trashed + completed (Trash takes priority? — оба predicates true,
  // но в counts оба inc'аются, в filter — оба list'а содержат).
  mk("trashed-completed", {
    isCompleted: true,
    completedAt: "2026-05-14T10:00:00.000Z",
    isTrashed: true,
  }),
];

/** Expected outputs для каждого SmartList. Order matters — see sorters. */
export const EXPECTED: Record<string, string[]> = {
  // Inbox: !projectId && !isSomeday && active. Sort by sortOrder asc.
  // Note: Inbox НЕ фильтрует по scheduledDate / isToday — anything that's
  // not in a project, not someday, active попадает. Поэтому upcoming-1/2
  // (no project, scheduled future) и today-flag/today-scheduled тоже Inbox.
  inbox: [
    "inbox-1",
    "inbox-2",
    "today-scheduled",
    "today-flag",
    "upcoming-2",
    "upcoming-1",
  ],

  // Today: active && (isToday || scheduledDate=today). Sort sortOrder asc.
  // today-scheduled(5), today-flag(10). scheduled-today-completed — НЕ
  // active. trashed-completed — НЕ active.
  today: ["today-scheduled", "today-flag"],

  // Upcoming: scheduledDate set && active && !someday. Sort scheduledDate asc.
  // today-scheduled(2026-05-15), upcoming-2(2026-05-20), proj-task(2026-05-20),
  // upcoming-1(2026-06-01). proj-task vs upcoming-2: одинаковая дата, тогда
  // stable sort по input order (JS Array.toSorted preserves order on ties)?
  // Реально localeCompare returns 0, sort stable — order = упоминание в FIXTURES.
  // upcoming-1, upcoming-2 ДО proj-task в FIXTURES? Нет — proj-task позднее.
  // FIXTURES order: upcoming-1, upcoming-2, ..., proj-task. Same-date items:
  // upcoming-2 (idx 5), proj-task (idx 9) → upcoming-2 первый.
  upcoming: ["today-scheduled", "upcoming-2", "proj-task", "upcoming-1"],

  // Anytime: active && !someday. Sort sortOrder asc.
  // inbox-1(1), inbox-2(2), today-scheduled(5), proj-task(7), today-flag(10),
  // upcoming-2(99), upcoming-1(100).
  anytime: [
    "inbox-1",
    "inbox-2",
    "today-scheduled",
    "proj-task",
    "today-flag",
    "upcoming-2",
    "upcoming-1",
  ],

  // Someday: isSomeday && !completed && !cancelled && !trashed. sortOrder asc.
  someday: ["someday-1", "someday-2"],

  // Logbook: completed || cancelled. Sort by completedAt|cancelledAt desc.
  // completed-2(2026-05-15T08), trashed-completed(2026-05-14T10),
  // completed-1(2026-05-14T12), scheduled-today-completed(2026-05-15T10),
  // cancelled-1(2026-05-13T15).
  // Desc by date: 2026-05-15T10 > 2026-05-15T08 > 2026-05-14T12 > 2026-05-14T10 > 2026-05-13T15.
  logbook: [
    "scheduled-today-completed",
    "completed-2",
    "completed-1",
    "trashed-completed",
    "cancelled-1",
  ],

  // Trash: isTrashed. Sort by createdAt desc.
  // trash-2(2026-05-15T11), trashed-completed(2026-05-15T10 default),
  // trash-1(2026-05-12T10). trashed-completed createdAt = default
  // 2026-05-15T10:00 → middle.
  trash: ["trash-2", "trashed-completed", "trash-1"],
};

/** Expected counts (countAll). */
export const EXPECTED_COUNTS: Record<string, number> = {
  inbox: 6,
  today: 2,
  upcoming: 4,
  anytime: 7,
  someday: 2,
  logbook: 5,
  trash: 3,
};

/** Synthetic benchmark dataset — pure-deterministic.
 *  Distribution выбран так чтобы все SmartList'ы non-trivially заполнены:
 *  ~25% trashed, ~25% completed, ~10% someday, ~10% today, ~30% inbox-ish. */
export function makeSynthetic(n: number): TodoItem[] {
  const out: TodoItem[] = [];
  for (let i = 0; i < n; i++) {
    const bucket = i % 10;
    const base = mk(`syn-${i}`, {
      sortOrder: i,
      createdAt: `2026-05-${String((i % 28) + 1).padStart(2, "0")}T10:00:00.000Z`,
    });
    if (bucket < 3) {
      // inbox-ish: no project, active
    } else if (bucket === 3) {
      base.isToday = true;
    } else if (bucket === 4) {
      base.scheduledDate = TODAY_ISO;
    } else if (bucket === 5) {
      base.scheduledDate = "2026-06-01";
    } else if (bucket === 6) {
      base.isSomeday = true;
    } else if (bucket === 7) {
      base.isCompleted = true;
      base.completedAt = base.createdAt;
    } else if (bucket === 8) {
      base.isTrashed = true;
    } else {
      base.projectId = "proj-bench";
    }
    out.push(base);
  }
  return out;
}
