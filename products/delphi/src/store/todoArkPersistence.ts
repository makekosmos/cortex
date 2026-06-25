import { toRaw } from "vue";
import type { TodoItem } from "@/types/task";

function getElectronInvoker(): {
  invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
} | null {
  if (
    typeof window === "undefined" ||
    !(window as unknown as Record<string, unknown>).electronAPI
  ) {
    return null;
  }

  const electronAPI = (
    window as unknown as Record<
      string,
      { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> }
    >
  ).electronAPI;

  return electronAPI ?? null;
}

export function persistTodoToArk(todo: TodoItem): void {
  const electronAPI = getElectronInvoker();
  if (!electronAPI) return;

  electronAPI.invoke("ark:upsertDelphiTask", cloneTodoForArk(todo)).catch(() => {});
}

export function deleteTodoFromArk(id: string): void {
  const electronAPI = getElectronInvoker();
  if (!electronAPI) return;

  electronAPI.invoke("ark:deleteDelphiTask", id).catch(() => {});
}

function cloneTodoForArk(todo: TodoItem): TodoItem {
  const rawTodo = toRaw(todo);
  if (typeof structuredClone === "function") {
    return structuredClone(rawTodo);
  }

  return {
    ...rawTodo,
    tagIds: [...rawTodo.tagIds],
    checklistItems: rawTodo.checklistItems.map((item) => ({ ...item })),
    recurrenceRule: rawTodo.recurrenceRule
      ? {
          ...rawTodo.recurrenceRule,
          daysOfWeek: rawTodo.recurrenceRule.daysOfWeek
            ? [...rawTodo.recurrenceRule.daysOfWeek]
            : rawTodo.recurrenceRule.daysOfWeek,
        }
      : rawTodo.recurrenceRule,
  };
}
