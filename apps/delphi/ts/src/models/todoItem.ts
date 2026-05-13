import { type ChecklistItem, Priority, type TodoItem } from "@/types/task";

const uuid = () => crypto.randomUUID();

// ---------------------------------------------------------------------------

// Factory

// ---------------------------------------------------------------------------

export type CreateTodoParams = {
  title: string;

  notes?: string | null;

  priority?: Priority;

  scheduledDate?: string | null;

  deadline?: string | null;

  reminderDate?: string | null;

  isToday?: boolean;

  isEvening?: boolean;

  isSomeday?: boolean;

  projectId?: string | null;

  areaId?: string | null;

  headingId?: string | null;

  billable?: boolean;

  price?: number | null;
};

export function createTodoItem(params: CreateTodoParams): TodoItem {
  return {
    id: uuid(),

    title: params.title,

    notes: params.notes ?? null,

    priority: params.priority ?? Priority.None,

    scheduledDate: params.scheduledDate ?? null,

    deadline: params.deadline ?? null,

    reminderDate: params.reminderDate ?? null,

    isToday: params.isToday ?? false,

    isEvening: params.isEvening ?? false,

    isSomeday: params.isSomeday ?? false,

    isCompleted: false,

    completedAt: null,

    isCancelled: false,

    cancelledAt: null,

    isTrashed: false,

    sortOrder: 0,

    createdAt: new Date().toISOString(),

    headingId: params.headingId ?? null,

    projectId: params.projectId ?? null,

    areaId: params.areaId ?? null,

    tagIds: [],

    checklistItems: [],

    recurrenceRule: null,

    billable: params.billable ?? false,

    price: params.price ?? null,
  };
}

// ---------------------------------------------------------------------------

// State transitions (immutable — return new objects)

// ---------------------------------------------------------------------------

export function markCompleted(todo: TodoItem): TodoItem {
  return {
    ...todo,

    isCompleted: true,

    completedAt: new Date().toISOString(),

    isCancelled: false,

    cancelledAt: null,
  };
}

export function markIncomplete(todo: TodoItem): TodoItem {
  return {
    ...todo,

    isCompleted: false,

    completedAt: null,

    isCancelled: false,

    cancelledAt: null,
  };
}

export function markCancelled(todo: TodoItem): TodoItem {
  return {
    ...todo,

    isCancelled: true,

    cancelledAt: new Date().toISOString(),

    isCompleted: false,

    completedAt: null,
  };
}

export function moveToTrash(todo: TodoItem): TodoItem {
  return { ...todo, isTrashed: true };
}

export function restoreFromTrash(todo: TodoItem): TodoItem {
  return { ...todo, isTrashed: false };
}

// ---------------------------------------------------------------------------

// Checklist helpers

// ---------------------------------------------------------------------------

export function createChecklistItem(
  title: string,

  todoItemId?: string,
): ChecklistItem {
  return {
    id: uuid(),

    title,

    isCompleted: false,

    sortOrder: 0,

    todoItemId: todoItemId ?? null,
  };
}

export function addChecklistItem(todo: TodoItem, title: string): TodoItem {
  const item = createChecklistItem(title, todo.id);

  item.sortOrder = todo.checklistItems.length;

  return {
    ...todo,

    checklistItems: [...todo.checklistItems, item],
  };
}

export function toggleChecklistItem(todo: TodoItem, itemId: string): TodoItem {
  return {
    ...todo,

    checklistItems: todo.checklistItems.map((ci) =>
      ci.id === itemId ? { ...ci, isCompleted: !ci.isCompleted } : ci,
    ),
  };
}

export function removeChecklistItem(todo: TodoItem, itemId: string): TodoItem {
  return {
    ...todo,

    checklistItems: todo.checklistItems.filter((ci) => ci.id !== itemId),
  };
}

export function reorderChecklistItems(
  todo: TodoItem,

  orderedIds: string[],
): TodoItem {
  const map = new Map(todo.checklistItems.map((ci) => [ci.id, ci]));

  const reordered = orderedIds

    .map((id, idx) => {
      const item = map.get(id);

      return item ? { ...item, sortOrder: idx } : null;
    })

    .filter((x): x is ChecklistItem => x !== null);

  return { ...todo, checklistItems: reordered };
}
