import { randomUUID } from "expo-crypto";
import { ProjectStatus, type Project, type TodoItem } from "@/types/task";

export type CreateProjectParams = {
  title: string;
  notes?: string | null;
  status?: ProjectStatus;
  areaId?: string | null;
  colorTag?: string | null;
};

export function createProject(params: CreateProjectParams): Project {
  return {
    id: randomUUID(),
    title: params.title,
    notes: params.notes ?? null,
    status: params.status ?? ProjectStatus.Active,
    scheduledDate: null,
    deadline: null,
    sortOrder: 0,
    colorTag: params.colorTag ?? null,
    createdAt: new Date().toISOString(),
    areaId: params.areaId ?? null,
  };
}

export function completedCount(todos: TodoItem[]): number {
  return todos.filter((t) => t.isCompleted).length;
}

export function totalCount(todos: TodoItem[]): number {
  return todos.filter((t) => !t.isTrashed).length;
}

export function progress(todos: TodoItem[]): number {
  const total = totalCount(todos);
  if (total === 0) return 0;
  return completedCount(todos) / total;
}
