/**
 * Local SQLite database bridge (Rust sidecar via Electron IPC).
 * Only available in Electron — web builds fall back to Ark HTTP API.
 */

import type { Area, Project, Tag, TodoItem } from "@/types/task";

function db() {
  return (window as unknown as Record<string, unknown>).electronAPI as
    | { db: ElectronDb }
    | undefined;
}

interface ElectronDb {
  loadAll: () => Promise<{
    todos: TodoItem[];

    projects: Project[];

    areas: Area[];

    tags: Tag[];

    headings: unknown[];
  }>;

  upsertTodo: (todo: TodoItem) => Promise<void>;

  deleteTodo: (id: string) => Promise<void>;

  batchUpsertTodos: (todos: TodoItem[]) => Promise<void>;

  upsertProject: (project: Project) => Promise<void>;

  deleteProject: (id: string) => Promise<void>;

  getSyncKv: (key: string) => Promise<string | null>;

  setSyncKv: (key: string, value: string) => Promise<void>;

  clearAll: () => Promise<void>;

  deleteTrashed: () => Promise<number>;
}

export function isLocalDbAvailable(): boolean {
  return !!db()?.db;
}

export async function loadAllFromLocalDb() {
  const result = await db()!.db.loadAll();

  return result;
}

export async function localDbUpsertTodo(todo: TodoItem): Promise<void> {
  await db()?.db.upsertTodo(JSON.parse(JSON.stringify(todo)));
}

export async function localDbDeleteTodo(id: string): Promise<void> {
  await db()?.db.deleteTodo(id);
}

export async function localDbBatchUpsertTodos(
  todos: TodoItem[],
): Promise<void> {
  if (todos.length === 0) return;

  await db()?.db.batchUpsertTodos(todos);
}

export async function localDbUpsertProject(project: Project): Promise<void> {
  await db()?.db.upsertProject(project);
}

export async function localDbDeleteProject(id: string): Promise<void> {
  await db()?.db.deleteProject(id);
}

export async function localDbGetSyncKv(key: string): Promise<string | null> {
  return (await db()?.db.getSyncKv(key)) ?? null;
}

export async function localDbSetSyncKv(
  key: string,

  value: string,
): Promise<void> {
  await db()?.db.setSyncKv(key, value);
}

export async function localDbClearAll(): Promise<void> {
  if (!isLocalDbAvailable()) return;

  await db()!.db.clearAll();
}

export async function localDbDeleteTrashed(): Promise<number> {
  if (!isLocalDbAvailable()) return 0;

  return await db()!.db.deleteTrashed();
}
