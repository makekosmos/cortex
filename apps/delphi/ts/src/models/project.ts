import { ProjectStatus, type Project, type TodoItem } from "@/types/task";
const uuid = () => crypto.randomUUID();

export type CreateProjectParams = {
  title: string;
  notes?: string | null;
  status?: ProjectStatus;
  areaId?: string | null;
  colorTag?: string | null;
};

export function createProject(params: CreateProjectParams): Project {
  return {
    id: uuid(),
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

/** Computed: count of completed (non-trashed) todos in a project. */
export function completedCount(todos: TodoItem[]): number {
  return todos.filter((t) => t.isCompleted).length;
}

/** Computed: count of non-trashed todos in a project. */
export function totalCount(todos: TodoItem[]): number {
  return todos.filter((t) => !t.isTrashed).length;
}

/** Computed: progress 0..1 for a project. */
export function progress(todos: TodoItem[]): number {
  const total = totalCount(todos);
  if (total === 0) return 0;
  return completedCount(todos) / total;
}
