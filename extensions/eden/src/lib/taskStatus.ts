// Linear-style task statuses.
//
// 7 значений по жизненному циклу задачи: triage → backlog/todo/in_progress
// → done/canceled/duplicate. Хранится в `task_obj.propsJson.status`.
//
// Совместимость с Delphi: is_completed / is_cancelled остаются в propsJson
// (Delphi UI читает их). Мы их derive'им из status: done → is_completed,
// canceled/duplicate → is_cancelled. Pattern на запись — single source of
// truth status, derived flags пишутся синхронно.

export const TASK_STATUSES = [
  "triage",
  "backlog",
  "todo",
  "in_progress",
  "done",
  "canceled",
] as const;

export type TaskStatus = (typeof TASK_STATUSES)[number];

export const TASK_STATUS_DEFAULT: TaskStatus = "todo";

export const TASK_STATUS_LABELS: Record<TaskStatus, string> = {
  triage: "Сортировка",
  backlog: "Бэклог",
  todo: "К выполнению",
  in_progress: "В работе",
  done: "Готово",
  canceled: "Отменена",
};

/**
 * Категория для семантики UI:
 * - "open" — задача в активном жизненном цикле, не done и не canceled.
 *   Title без strikethrough, checkbox outline.
 * - "completed" — done. Title затемнён. Checkbox filled accent. Опасити
 *   опускается на весь row.
 * - "terminated" — canceled. Title strikethrough. Checkbox filled gray.
 */
export type TaskStatusCategory = "open" | "completed" | "terminated";

export function getStatusCategory(status: TaskStatus): TaskStatusCategory {
  if (status === "done") return "completed";
  if (status === "canceled") return "terminated";
  return "open";
}

/**
 * Нормализует значение из ARK (может быть unknown / undefined / legacy
 * tasks без status). Fallback к derive из is_completed/is_cancelled:
 *   - is_completed=true → done
 *   - is_cancelled=true → canceled
 *   - else → todo (default)
 *
 * Legacy: до 2026-05-20 был статус "duplicate". Если встретим его в
 * старых tasks — нормализуем к "canceled" (та же семантика terminal-fail).
 */
export function normalizeStatus(input: {
  status?: unknown;
  is_completed?: unknown;
  is_cancelled?: unknown;
}): TaskStatus {
  if (typeof input.status === "string") {
    if ((TASK_STATUSES as readonly string[]).includes(input.status)) {
      return input.status as TaskStatus;
    }
    if (input.status === "duplicate") return "canceled";
  }
  if (input.is_completed === true) return "done";
  if (input.is_cancelled === true) return "canceled";
  return TASK_STATUS_DEFAULT;
}
