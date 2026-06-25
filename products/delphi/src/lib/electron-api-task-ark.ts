import type { TodoItem } from "@/types/task";
import {
  arkTaskObjectToTodo,
  asBoolean,
  asNullableString,
  asObject,
  DELPHI_TASK_OBJECT_TYPE_ID,
  ensureTaskObjectTypeRegistered,
  todoToArkTaskObject,
  type ArkObjectRecord,
  type KeplerArk,
} from "./electron-api-task-mapping";

export { DELPHI_TASK_OBJECT_TYPE_ID, ensureTaskObjectTypeRegistered, type KeplerArk };

export interface TimeEntrySummary {
  id: string;
  taskId: string | null;
  billable: boolean;
  startedAt: string;
  endedAt: string | null;
}

export function kepler(): KeplerArk | null {
  const k = (window as unknown as { kepler?: { ark: KeplerArk } }).kepler;
  return k ? k.ark : null;
}

export async function arkListTasks(): Promise<TodoItem[]> {
  const ark = kepler();
  if (!ark) return [];
  const list = await ark.request<ArkObjectRecord[]>("list_objects_by_type", {
    type_id: DELPHI_TASK_OBJECT_TYPE_ID,
  });
  return Array.isArray(list)
    ? list
        .filter((o) => o.typeId === DELPHI_TASK_OBJECT_TYPE_ID && !o.deletedAt)
        .map(arkTaskObjectToTodo)
    : [];
}

export async function arkUpsertTask(todo: TodoItem): Promise<boolean> {
  const ark = kepler();
  if (!ark) return false;
  try {
    await ensureTaskObjectTypeRegistered(ark);
    await ark.request("upsert_object", { object: todoToArkTaskObject(todo) });
    return true;
  } catch (err) {
    console.warn("[delphi-extension] arkUpsertTask failed:", err);
    return false;
  }
}

export async function arkDeleteTask(id: string): Promise<boolean> {
  const ark = kepler();
  if (!ark) return false;
  await ark.request("delete_object", { id });
  return true;
}

export async function arkGetTask(id: string): Promise<TodoItem | null> {
  const ark = kepler();
  if (!ark) return null;
  try {
    const obj = await ark.request<ArkObjectRecord | null>("get_object", { id });
    if (!obj || obj.typeId !== DELPHI_TASK_OBJECT_TYPE_ID) return null;
    if (obj.deletedAt) return null;
    return arkTaskObjectToTodo(obj);
  } catch (err) {
    console.warn("[delphi-extension] arkGetTask failed:", err);
    return null;
  }
}

export async function arkListTimeEntries(): Promise<TimeEntrySummary[]> {
  const ark = kepler();
  if (!ark) return [];
  const list = await ark.request<ArkObjectRecord[]>("list_objects_by_type", {
    type_id: "time_entry_obj",
  });
  if (!Array.isArray(list)) return [];
  return list
    .filter((o) => !o.deletedAt)
    .map((o) => {
      const props = asObject(o.propsJson);
      return {
        id: o.id,
        taskId: asNullableString(props.taskId),
        billable: asBoolean(props.billable, false),
        startedAt: typeof props.startedAt === "string" ? props.startedAt : "",
        endedAt: asNullableString(props.endedAt),
      };
    });
}
