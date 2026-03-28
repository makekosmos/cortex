import {
  Frequency,
  RecurrenceType,
  type RecurrenceData,
  type TodoItem,
} from "@/types/task";

const uuid = () => crypto.randomUUID();

// ---------------------------------------------------------------------------
// nextDate — compute next occurrence date from a RecurrenceData rule
// ---------------------------------------------------------------------------

export function nextDate(rule: RecurrenceData, after: Date): Date | null {
  if (rule.endDate) {
    const end = new Date(rule.endDate);
    if (after >= end) return null;
  }

  const base = new Date(after);

  switch (rule.frequency) {
    case Frequency.Daily: {
      base.setDate(base.getDate() + rule.interval);
      return base;
    }

    case Frequency.Weekly: {
      if (rule.daysOfWeek && rule.daysOfWeek.length > 0) {
        // JS: 0=Sunday, matches Swift weekday numbering (1=Sunday) offset by -1
        // Swift uses Calendar weekday where 1=Sunday. We store 1-based values.
        const weekday = base.getDay() + 1; // convert JS 0-based to Swift 1-based
        const sorted = rule.daysOfWeek.toSorted((a, b) => a - b);

        // Find next day in the current week
        const nextDay = sorted.find((d) => d > weekday);
        if (nextDay !== undefined) {
          base.setDate(base.getDate() + (nextDay - weekday));
          return base;
        }

        // Wrap to first day of next interval week
        const first = sorted[0];
        if (first !== undefined) {
          const daysUntil = 7 * rule.interval - weekday + first;
          base.setDate(base.getDate() + daysUntil);
          return base;
        }
      }

      // No specific days — just add N weeks
      base.setDate(base.getDate() + 7 * rule.interval);
      return base;
    }

    case Frequency.Monthly: {
      base.setMonth(base.getMonth() + rule.interval);
      return base;
    }

    case Frequency.Yearly: {
      base.setFullYear(base.getFullYear() + rule.interval);
      return base;
    }
  }
}

// ---------------------------------------------------------------------------
// duplicateTodo — clone a todo (new id, reset completion state)
// ---------------------------------------------------------------------------

export function duplicateTodo(todo: TodoItem): TodoItem {
  return {
    ...todo,
    id: uuid(),
    isCompleted: false,
    completedAt: null,
    isCancelled: false,
    cancelledAt: null,
    isTrashed: false,
    createdAt: new Date().toISOString(),
    checklistItems: todo.checklistItems.map((ci) => ({
      ...ci,
      id: uuid(),
      isCompleted: false,
    })),
  };
}

// ---------------------------------------------------------------------------
// createNextRecurrence — produce the next recurring todo (or null)
// ---------------------------------------------------------------------------

export function createNextRecurrence(todo: TodoItem): TodoItem | null {
  const rule = todo.recurrenceRule;
  if (!rule) return null;

  let baseDate: Date;
  switch (rule.recurrenceType) {
    case RecurrenceType.Fixed:
      baseDate = todo.scheduledDate ? new Date(todo.scheduledDate) : new Date();
      break;
    case RecurrenceType.AfterCompletion:
      baseDate = todo.completedAt ? new Date(todo.completedAt) : new Date();
      break;
  }

  const next = nextDate(rule, baseDate);
  if (!next) return null;

  const copy = duplicateTodo(todo);
  copy.scheduledDate = next.toISOString();
  copy.isToday = false;
  copy.isEvening = false;

  return copy;
}
