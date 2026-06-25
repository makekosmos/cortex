// Bridge legacy `window.electronAPI.*` calls over `window.kepler.ark.request(...)`.
//
// Delphi still has legacy call-sites that expect `window.electronAPI.invoke(...)`.
// In the extension renderer there is no direct Electron IPC, only the ARK bridge,
// so this shim keeps the minimal compatibility surface for task CRUD and live sync.

import type { Area, Project, Tag, TodoItem } from "@/types/task";
import {
  arkDeleteTask,
  arkGetTask,
  arkListTasks,
  arkListTimeEntries,
  arkUpsertTask,
  DELPHI_TASK_OBJECT_TYPE_ID,
  ensureTaskObjectTypeRegistered,
  kepler,
} from "./electron-api-task-ark";

export { arkGetTask } from "./electron-api-task-ark";
export const DELPHI_TASK_OBJ_TYPE_ID = DELPHI_TASK_OBJECT_TYPE_ID;

export interface ArkTaskChange {
  event: "object_upserted" | "object_deleted";
  id: string;
  typeId?: string;
}

export function subscribeArkObjectChanges(handler: (change: ArkTaskChange) => void): () => void {
  const ark = kepler();
  if (!ark) return () => {};
  const offU = ark.subscribe("object_upserted", (payload) => {
    if (!payload || typeof payload !== "object") return;
    const p = payload as { id?: unknown; type_id?: unknown };
    if (typeof p.id !== "string") return;
    handler({
      event: "object_upserted",
      id: p.id,
      typeId: typeof p.type_id === "string" ? p.type_id : undefined,
    });
  });
  const offD = ark.subscribe("object_deleted", (payload) => {
    if (!payload || typeof payload !== "object") return;
    const p = payload as { id?: unknown };
    if (typeof p.id !== "string") return;
    handler({ event: "object_deleted", id: p.id });
  });
  return () => {
    offU();
    offD();
  };
}

const warnedChannels = new Set<string>();

function warnOnce(channel: string, detail?: string): void {
  if (warnedChannels.has(channel)) return;
  warnedChannels.add(channel);
  console.warn(
    `[delphi-extension] electronAPI channel '${channel}' is unavailable in extension; using no-op${detail ? ` (${detail})` : ""}`,
  );
}

async function invokeChannel(channel: string, args: unknown[]): Promise<unknown> {
  switch (channel) {
    case "db:loadAll":
    case "ark:listDelphiTasks": {
      const todos = await arkListTasks();
      if (channel === "db:loadAll") {
        return {
          todos,
          projects: [] as Project[],
          areas: [] as Area[],
          tags: [] as Tag[],
          headings: [],
        };
      }
      return todos;
    }

    case "db:upsertTodo":
    case "ark:upsertDelphiTask":
      return arkUpsertTask(args[0] as TodoItem);

    case "db:deleteTodo":
    case "ark:deleteDelphiTask":
      return arkDeleteTask(args[0] as string);

    case "db:batchUpsertTodos": {
      if (!Array.isArray(args[0])) throw new Error("db:batchUpsertTodos expects TodoItem[]");
      const todos = args[0] as TodoItem[];
      for (const todo of todos) await arkUpsertTask(todo);
      return true;
    }

    case "ark:listTimeEntries":
      return arkListTimeEntries();

    case "db:upsertProject":
    case "db:deleteProject":
    case "db:getSyncKv":
    case "db:setSyncKv":
    case "db:clearAll":
    case "db:deleteTrashed":
      warnOnce(channel);
      return channel === "db:getSyncKv" ? null : true;

    default:
      warnOnce(channel, "unknown channel");
      return null;
  }
}

function subscribeChannel(channel: string, _handler: (...args: unknown[]) => void): () => void {
  const ark = kepler();
  if (!ark) return () => {};

  switch (channel) {
    case "delphi:cmd:task:create":
    case "delphi:cmd:task:today":
      return () => {};

    default:
      warnOnce(`on:${channel}`, "unknown event");
      return () => {};
  }
}

const electronApiShim = {
  invoke: (channel: string, ...args: unknown[]): Promise<unknown> => invokeChannel(channel, args),
  on: (channel: string, handler: (...args: unknown[]) => void): (() => void) =>
    subscribeChannel(channel, handler),
};

const w = window as unknown as Record<string, unknown>;
if (!w.electronAPI) {
  w.electronAPI = electronApiShim;
}

const initialArk = kepler();
if (initialArk) {
  void ensureTaskObjectTypeRegistered(initialArk).catch(() => {
    // First real upsert retries registration; boot-time registration is best effort.
  });
}
