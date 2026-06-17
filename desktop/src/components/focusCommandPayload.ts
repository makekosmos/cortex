import type { FocusBlockedApp, StartFocusSessionInput } from "@shared/ipc-types";

interface BuildFocusSessionStartInputOptions {
  title: string;
  durationMin: number;
  taskId: string | null;
  taskTitle: string | null;
  categoryIds: readonly string[];
  blockedAppIds?: readonly string[];
  blockedApps?: readonly FocusBlockedApp[];
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
    blockedAppIds: Array.from(options.blockedAppIds ?? []),
    blockedApps: Array.from(options.blockedApps ?? []).map((app) => ({
      id: app.id,
      name: app.name,
      icon: app.icon ?? null,
      exec_path: app.exec_path ?? null,
    })),
  };
}
