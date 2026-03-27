import { SmartList, type TodoItem } from '@/types/task';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function isDateToday(iso: string | null | undefined): boolean {
  if (!iso) return false;
  const d = new Date(iso);
  const now = new Date();
  return (
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  );
}

/** Base predicates shared between count() and filter(). */
function isActive(t: TodoItem): boolean {
  return !t.isCompleted && !t.isCancelled && !t.isTrashed;
}

// ---------------------------------------------------------------------------
// Filter predicates per smart list
// ---------------------------------------------------------------------------

const predicates: Record<SmartList, (t: TodoItem) => boolean> = {
  [SmartList.Inbox]: (t) =>
    !t.projectId && !t.isSomeday && isActive(t),

  [SmartList.Today]: (t) =>
    isActive(t) && (t.isToday || isDateToday(t.scheduledDate)),

  [SmartList.Upcoming]: (t) =>
    !!t.scheduledDate && isActive(t) && !t.isSomeday,

  [SmartList.Anytime]: (t) =>
    isActive(t) && !t.isSomeday,

  [SmartList.Someday]: (t) =>
    t.isSomeday && !t.isCompleted && !t.isCancelled && !t.isTrashed,

  [SmartList.Logbook]: (t) =>
    t.isCompleted || t.isCancelled,

  [SmartList.Trash]: (t) =>
    t.isTrashed,
};

// ---------------------------------------------------------------------------
// Sort comparators per smart list
// ---------------------------------------------------------------------------

function byCreatedAtDesc(a: TodoItem, b: TodoItem): number {
  return (b.createdAt ?? '').localeCompare(a.createdAt ?? '');
}

function bySortOrderAsc(a: TodoItem, b: TodoItem): number {
  return a.sortOrder - b.sortOrder;
}

function byScheduledDateAsc(a: TodoItem, b: TodoItem): number {
  const da = a.scheduledDate ?? '\uffff';
  const db = b.scheduledDate ?? '\uffff';
  return da.localeCompare(db);
}

function byCompletionDesc(a: TodoItem, b: TodoItem): number {
  const da = a.completedAt ?? a.cancelledAt ?? '';
  const db = b.completedAt ?? b.cancelledAt ?? '';
  return db.localeCompare(da);
}

const sorters: Record<SmartList, (a: TodoItem, b: TodoItem) => number> = {
  [SmartList.Inbox]: byCreatedAtDesc,
  [SmartList.Today]: bySortOrderAsc,
  [SmartList.Upcoming]: byScheduledDateAsc,
  [SmartList.Anytime]: byCreatedAtDesc,
  [SmartList.Someday]: byCreatedAtDesc,
  [SmartList.Logbook]: byCompletionDesc,
  [SmartList.Trash]: byCreatedAtDesc,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export function filterTodos(list: SmartList, todos: TodoItem[]): TodoItem[] {
  return todos.filter(predicates[list]).toSorted(sorters[list]);
}

export function countTodos(list: SmartList, todos: TodoItem[]): number {
  return todos.filter(predicates[list]).length;
}

/** Count all smart lists at once (avoids repeated iterations). */
export function countAll(todos: TodoItem[]): Record<SmartList, number> {
  const counts = {} as Record<SmartList, number>;
  for (const list of Object.values(SmartList)) {
    counts[list] = 0;
  }
  for (const todo of todos) {
    for (const list of Object.values(SmartList)) {
      if (predicates[list](todo)) {
        counts[list]++;
      }
    }
  }
  return counts;
}
