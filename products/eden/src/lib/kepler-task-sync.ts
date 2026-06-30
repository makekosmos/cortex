import { writeEntryMarkdown } from "@/editor-content/content";
import { parseNoteTypeDefinition, parseNoteTypeUiSchema } from "@/lib/typedNotes";
import { normalizeStatus, type TaskStatus } from "./taskStatus";

const EMPTY_PROPS: Record<string, never> = Object.freeze({});
const EDEN_EMPTY_TASK_TITLE = "Пустая задача";

export const EDEN_TASK_OBJECT_TYPE_ID = "task_obj";

const TASK_OBJECT_TYPE_SCHEMA_JSON = JSON.stringify({
  fields: [
    {
      id: "deadline",
      label: "Дедлайн",
      kind: "date",
      required: false,
      visible: true,
      read_only: false,
    },
  ],
});

const TASK_OBJECT_TYPE_UI_SCHEMA_JSON = JSON.stringify({
  featured_fields: ["deadline"],
  visible_fields: ["deadline"],
  hidden_fields: ["created_at", "updated_at", "deleted_at"],
  read_only_fields: [],
  field_order: ["deadline"],
  header_layout: "inline",
  default_layout: "page",
  collection_name: "Задачи",
});

export interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

interface KeplerArkBridge {
  request: <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;
  subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
}

interface KeplerNamespace {
  ark: KeplerArkBridge;
}

function keplerBridge(): KeplerArkBridge {
  const k = (window as unknown as { kepler?: KeplerNamespace }).kepler;
  if (!k?.ark) {
    throw new Error("Eden extension: window.kepler.ark is unavailable");
  }
  return k.ark;
}

function ark<T = unknown>(operation: string, params?: Record<string, unknown>): Promise<T> {
  return keplerBridge().request<T>(operation, params);
}

function millisToArkTimestamp(value: number | null | undefined): string {
  return new Date(value ?? Date.now()).toISOString();
}

export function normalizeTaskObjectTypeSchemaJson(schemaJson: string): string {
  try {
    parseNoteTypeDefinition(schemaJson);
    return schemaJson;
  } catch {
    return TASK_OBJECT_TYPE_SCHEMA_JSON;
  }
}

export function normalizeTaskObjectTypeUiSchemaJson(uiSchemaJson: string): string {
  try {
    const uiSchema = parseNoteTypeUiSchema(uiSchemaJson);
    if (
      uiSchema.featured_fields?.includes("deadline") &&
      uiSchema.visible_fields?.includes("deadline")
    ) {
      return uiSchemaJson;
    }
  } catch {
    // fall through to canonical task metadata
  }
  return TASK_OBJECT_TYPE_UI_SCHEMA_JSON;
}

let taskObjectTypeRegisterPromise: Promise<void> | null = null;

export function ensureTaskObjectTypeRegistered(): Promise<void> {
  if (taskObjectTypeRegisterPromise) return taskObjectTypeRegisterPromise;
  const now = new Date().toISOString();
  taskObjectTypeRegisterPromise = ark("upsert_object_type", {
    object_type: {
      id: EDEN_TASK_OBJECT_TYPE_ID,
      name: "Задача",
      schemaJson: TASK_OBJECT_TYPE_SCHEMA_JSON,
      uiSchemaJson: TASK_OBJECT_TYPE_UI_SCHEMA_JSON,
      systemLocked: false,
      createdAt: now,
      updatedAt: now,
    },
  })
    .then(() => undefined)
    .catch((err) => {
      taskObjectTypeRegisterPromise = null;
      throw err;
    });
  return taskObjectTypeRegisterPromise;
}

export async function getTask(taskId: string): Promise<ArkObjectRecord | null> {
  const obj = await ark<ArkObjectRecord | null>("get_object", { id: taskId });
  if (!obj) return null;
  if (obj.typeId !== EDEN_TASK_OBJECT_TYPE_ID) return null;
  if (obj.deletedAt) return null;
  return obj;
}

export async function patchTask(
  taskId: string,
  patch: {
    title?: string;
    isCompleted?: boolean;
    status?: TaskStatus;
  },
): Promise<void> {
  const existing = await ark<ArkObjectRecord | null>("get_object", { id: taskId });
  if (!existing) {
    console.warn("[eden-extension] patchTask: object not found", taskId);
    return;
  }
  const now = new Date().toISOString();
  const props = existing.propsJson ?? EMPTY_PROPS;
  const nextProps = { ...props };

  let nextStatus: TaskStatus;
  if (patch.status !== undefined) {
    nextStatus = patch.status;
  } else if (patch.isCompleted !== undefined) {
    nextStatus = patch.isCompleted ? "done" : "todo";
  } else {
    nextStatus = normalizeStatus({
      status: props.status,
      is_completed: props.is_completed,
      is_cancelled: props.is_cancelled,
    });
  }

  nextProps.status = nextStatus;
  nextProps.is_completed = deriveCompletedFlag(nextStatus);
  nextProps.is_cancelled = deriveCancelledFlag(nextStatus);
  if (deriveCompletedFlag(nextStatus) && !props.completed_at) {
    nextProps.completed_at = now;
  } else if (!deriveCompletedFlag(nextStatus)) {
    nextProps.completed_at = null;
  }
  if (deriveCancelledFlag(nextStatus) && !props.cancelled_at) {
    nextProps.cancelled_at = now;
  } else if (!deriveCancelledFlag(nextStatus)) {
    nextProps.cancelled_at = null;
  }

  const nextTitle =
    patch.title !== undefined ? patch.title.trim() || EDEN_EMPTY_TASK_TITLE : existing.title;
  await ark("upsert_object", {
    object: {
      ...existing,
      title: nextTitle,
      propsJson: nextProps,
      updatedAt: now,
    },
  });
}

export async function createTask(
  sourceNoteId: string,
  title = "",
  explicitId?: string,
): Promise<string> {
  await ensureTaskObjectTypeRegistered();
  const taskId = explicitId ?? crypto.randomUUID();
  const now = new Date().toISOString();
  const effectiveTitle = title.trim() || EDEN_EMPTY_TASK_TITLE;
  await ark("upsert_object", {
    object: {
      id: taskId,
      typeId: EDEN_TASK_OBJECT_TYPE_ID,
      title: effectiveTitle,
      contentJson: writeEntryMarkdown(""),
      propsJson: {
        description: null,
        priority: 0,
        scheduled_date: null,
        deadline: null,
        reminder_date: null,
        is_today: false,
        is_evening: false,
        is_someday: false,
        is_completed: false,
        completed_at: null,
        is_cancelled: false,
        cancelled_at: null,
        is_trashed: false,
        status: "triage",
        sort_order: 0,
        heading_id: null,
        project_id: null,
        area_id: null,
        tag_ids: [],
        checklist_items: [],
        recurrence_rule: null,
        billable: false,
        price: null,
        created_at: now,
        source_app: "eden",
        source_note_id: sourceNoteId,
        model_version: 1,
      },
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    },
  });
  return taskId;
}

export function subscribeObjectChanges(
  handler: (payload: {
    event: "object_upserted" | "object_deleted";
    id: string;
    typeId?: string;
  }) => void,
): () => void {
  const bridge = keplerBridge();
  const offU = bridge.subscribe("object_upserted", (payload) => {
    const p = payload as { id?: string; type_id?: string };
    if (typeof p.id === "string")
      handler({ event: "object_upserted", id: p.id, typeId: p.type_id });
  });
  const offD = bridge.subscribe("object_deleted", (payload) => {
    const p = payload as { id?: string };
    if (typeof p.id === "string") handler({ event: "object_deleted", id: p.id });
  });
  return () => {
    offU();
    offD();
  };
}

export async function softDeleteTask(taskId: string): Promise<void> {
  const existing = await ark<ArkObjectRecord | null>("get_object", { id: taskId });
  if (!existing || existing.deletedAt) return;
  const deletedAt = millisToArkTimestamp(Date.now());
  await ark("upsert_object", {
    object: {
      ...existing,
      updatedAt: deletedAt,
      deletedAt,
    },
  });
}

function deriveCompletedFlag(status: TaskStatus): boolean {
  return status === "done";
}

function deriveCancelledFlag(status: TaskStatus): boolean {
  return status === "canceled";
}
