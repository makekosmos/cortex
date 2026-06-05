import type { StartFocusSessionInput } from "@shared/ipc-types";

interface BuildFocusSessionStartInputOptions {
  title: string;
  durationMin: number;
  taskId: string | null;
  taskTitle: string | null;
  categoryIds: readonly string[];
}

export function buildFocusSessionStartInput(
  options: BuildFocusSessionStartInputOptions,
): StartFocusSessionInput {
  return {
    title: options.title,
    durationMin: options.durationMin,
    taskId: options.taskId,
    taskTitle: options.taskTitle,
    mode: "block",
    categoryIds: Array.from(options.categoryIds),
  };
}
