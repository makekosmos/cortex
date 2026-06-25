import type { TodoItem } from "@/types/task";

type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

export interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string | null;
  contentJson?: JsonValue;
  propsJson?: JsonValue;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

export interface KeplerArk {
  request: <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;
  subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
}

export const DELPHI_TASK_OBJECT_TYPE_ID = "task_obj";

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

let taskObjectTypeReady: Promise<void> | null = null;

export function ensureTaskObjectTypeRegistered(ark: KeplerArk): Promise<void> {
  if (taskObjectTypeReady) return taskObjectTypeReady;
  const now = isoNow();
  taskObjectTypeReady = ark
    .request("upsert_object_type", {
      object_type: {
        id: DELPHI_TASK_OBJECT_TYPE_ID,
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
      taskObjectTypeReady = null;
      throw err;
    });
  return taskObjectTypeReady;
}

export function todoToArkTaskObject(todo: TodoItem): ArkObjectRecord {
  const updatedAt = isoNow();
  const description = todo.notes ?? null;
  return {
    id: todo.id,
    typeId: DELPHI_TASK_OBJECT_TYPE_ID,
    title: todo.title,
    contentJson: textToArkContentJson(description),
    propsJson: {
      description,
      priority: todo.priority,
      scheduled_date: todo.scheduledDate ?? null,
      deadline: todo.deadline ?? null,
      reminder_date: todo.reminderDate ?? null,
      is_today: todo.isToday,
      is_evening: todo.isEvening,
      is_someday: todo.isSomeday,
      ...(todo.status ? { status: todo.status } : {}),
      is_completed: todo.isCompleted,
      completed_at: todo.completedAt ?? null,
      is_cancelled: todo.isCancelled,
      cancelled_at: todo.cancelledAt ?? null,
      is_trashed: todo.isTrashed,
      sort_order: todo.sortOrder,
      heading_id: todo.headingId ?? null,
      project_id: todo.projectId ?? null,
      area_id: todo.areaId ?? null,
      tag_ids: todo.tagIds,
      checklist_items: todo.checklistItems as unknown as JsonValue,
      recurrence_rule: (todo.recurrenceRule ?? null) as unknown as JsonValue,
      billable: todo.billable ?? false,
      price: todo.price ?? null,
      created_at: todo.createdAt,
      source_app: "delphi",
      model_version: 1,
    },
    createdAt: todo.createdAt,
    updatedAt,
    deletedAt: todo.isTrashed ? updatedAt : null,
  };
}

export function arkTaskObjectToTodo(object: ArkObjectRecord): TodoItem {
  const props = asObject(object.propsJson);
  const description =
    asNullableString(props.description) ?? extractPlainTextFromContentJson(object.contentJson);
  return {
    id: object.id.toLowerCase(),
    title: object.title ?? "",
    notes: description,
    priority: asNumber(props.priority, 0),
    scheduledDate: asNullableString(props.scheduled_date),
    deadline: asNullableString(props.deadline),
    reminderDate: asNullableString(props.reminder_date),
    isToday: asBoolean(props.is_today, false),
    isEvening: asBoolean(props.is_evening, false),
    isSomeday: asBoolean(props.is_someday, false),
    status: asNullableString(props.status),
    isCompleted: asBoolean(props.is_completed, false),
    completedAt: asNullableString(props.completed_at),
    isCancelled: asBoolean(props.is_cancelled, false),
    cancelledAt: asNullableString(props.cancelled_at),
    isTrashed: asBoolean(props.is_trashed, false) || Boolean(object.deletedAt),
    sortOrder: asNumber(props.sort_order, 0),
    createdAt: asNullableString(props.created_at) ?? object.createdAt,
    headingId: asNullableString(props.heading_id),
    projectId: asNullableString(props.project_id),
    areaId: asNullableString(props.area_id),
    tagIds: asStringArray(props.tag_ids),
    checklistItems: Array.isArray(props.checklist_items)
      ? (props.checklist_items as TodoItem["checklistItems"])
      : [],
    recurrenceRule:
      props.recurrence_rule && typeof props.recurrence_rule === "object"
        ? (props.recurrence_rule as TodoItem["recurrenceRule"])
        : null,
    billable: asBoolean(props.billable, false),
    price: typeof props.price === "number" && Number.isFinite(props.price) ? props.price : null,
  };
}

export function asObject(value: unknown): Record<string, unknown> {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  return {};
}

export function asBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

export function asNullableString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function isoNow(): string {
  return new Date().toISOString();
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === "string") : [];
}

function asNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function textToArkContentJson(text: string | null | undefined): JsonValue {
  const trimmed = (text ?? "").trim();
  if (!trimmed) {
    return { type: "doc", content: [{ type: "paragraph" }] };
  }
  return {
    type: "doc",
    content: [{ type: "paragraph", content: [{ type: "text", text: trimmed }] }],
  };
}

function extractPlainTextFromContentJson(value: unknown): string | null {
  const chunks: string[] = [];
  function visit(node: unknown) {
    if (!node) return;
    if (typeof node === "string") {
      chunks.push(node);
      return;
    }
    if (Array.isArray(node)) {
      node.forEach(visit);
      return;
    }
    if (typeof node !== "object") return;
    const record = node as Record<string, unknown>;
    if (typeof record.text === "string") chunks.push(record.text);
    if (Array.isArray(record.content)) {
      for (const child of record.content) visit(child);
      if (
        record.type === "paragraph" ||
        record.type === "heading" ||
        record.type === "blockquote" ||
        record.type === "listItem"
      ) {
        chunks.push("\n");
      }
    }
  }
  visit(value);
  const normalized = chunks
    .join("")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
  return normalized || null;
}
