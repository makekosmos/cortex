import type { Task } from "@/types/task";

import { isElectronRuntime } from "@/services/runtime/platform";

const TASKS_KEY = "todofus.tasks";

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

export function loadTasksFromWebStorage(): Task[] {
  if (isElectronRuntime()) return [];

  try {
    const raw = localStorage.getItem(TASKS_KEY);

    if (!raw) return [];

    const parsed = JSON.parse(raw);

    if (!Array.isArray(parsed)) return [];

    return parsed

      .filter(isObject)

      .map((task) => ({
        id: String(task.id ?? ""),

        title: String(task.title ?? ""),

        description: typeof task.description === "string" ? task.description : null,

        completed: Boolean(task.completed),

        priority: Number(task.priority ?? 0),

        due_date: typeof task.due_date === "string" ? task.due_date : null,

        list_id: typeof task.list_id === "string" ? task.list_id : null,

        user_id: typeof task.user_id === "string" ? task.user_id : undefined,

        created_at: new Date(String(task.created_at ?? Date.now())),

        updated_at: task.updated_at
          ? new Date(String(task.updated_at))
          : new Date(String(task.created_at ?? Date.now())),
      }))

      .filter((task) => task.id.length > 0);
  } catch {
    return [];
  }
}

export function saveTasksToWebStorage(tasks: Task[]) {
  if (isElectronRuntime()) return;

  const payload = tasks.map((task) => ({
    ...task,

    created_at: task.created_at.toISOString(),

    updated_at: task.updated_at ? task.updated_at.toISOString() : null,
  }));

  localStorage.setItem(TASKS_KEY, JSON.stringify(payload));
}
