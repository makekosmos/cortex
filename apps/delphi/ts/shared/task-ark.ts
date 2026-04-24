import type { TodoItem } from "../src/types/task";

export const DELPHI_TASK_OBJECT_TYPE_ID = "task_obj";
export const DELPHI_TASK_OBJECT_TYPE_NAME = "Задача";

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

export interface ArkObjectTypeRecord {
  id: string;
  name: string;
  schemaJson: string;
  uiSchemaJson: string;
  createdAt: string;
  updatedAt: string;
  systemLocked: boolean;
}

function isoNow(): string {
  return new Date().toISOString();
}

function textToArkContentJson(text: string | null | undefined): unknown {
  const trimmed = (text ?? "").trim();
  if (!trimmed) {
    return { type: "doc", content: [{ type: "paragraph" }] };
  }

  return {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [{ type: "text", text: trimmed }],
      },
    ],
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
      for (const item of node) {
        visit(item);
      }
      return;
    }

    if (typeof node !== "object") {
      return;
    }

    const record = node as Record<string, unknown>;
    if (typeof record.text === "string") {
      chunks.push(record.text);
    }

    if (Array.isArray(record.content)) {
      for (const child of record.content) {
        visit(child);
      }
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

  const normalized = chunks.join("").replace(/\n{3,}/g, "\n\n").trim();
  return normalized || null;
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter((item): item is string => typeof item === "string");
}

function asBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function asNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function asNullableString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function asChecklistItems(value: unknown): TodoItem["checklistItems"] {
  return Array.isArray(value) ? (value as TodoItem["checklistItems"]) : [];
}

function asRecurrenceRule(value: unknown): TodoItem["recurrenceRule"] {
  return value && typeof value === "object"
    ? (value as TodoItem["recurrenceRule"])
    : null;
}

export function createDelphiTaskObjectTypeRecord(
  now: string = isoNow(),
): ArkObjectTypeRecord {
  return {
    id: DELPHI_TASK_OBJECT_TYPE_ID,
    name: DELPHI_TASK_OBJECT_TYPE_NAME,
    schemaJson: JSON.stringify({
      fields: [
        {
          id: "description",
          label: "Описание",
          kind: "long_text",
          required: false,
          visible: true,
          read_only: false,
          system: false,
        },
        {
          id: "priority",
          label: "Приоритет",
          kind: "number",
          required: false,
          visible: true,
          read_only: false,
          system: false,
        },
        {
          id: "scheduled_date",
          label: "Запланировано",
          kind: "date",
          required: false,
          visible: true,
          read_only: false,
          system: false,
        },
        {
          id: "deadline",
          label: "Дедлайн",
          kind: "date",
          required: false,
          visible: true,
          read_only: false,
          system: false,
        },
        {
          id: "reminder_date",
          label: "Напоминание",
          kind: "date",
          required: false,
          visible: true,
          read_only: false,
          system: false,
        },
        {
          id: "is_today",
          label: "Сегодня",
          kind: "boolean",
          required: false,
          visible: true,
          read_only: false,
          system: false,
        },
        {
          id: "is_evening",
          label: "Вечер",
          kind: "boolean",
          required: false,
          visible: true,
          read_only: false,
          system: false,
        },
        {
          id: "is_completed",
          label: "Завершено",
          kind: "boolean",
          required: false,
          visible: true,
          read_only: false,
          system: false,
        },
        {
          id: "is_cancelled",
          label: "Отменено",
          kind: "boolean",
          required: false,
          visible: true,
          read_only: false,
          system: false,
        },
        {
          id: "is_trashed",
          label: "В корзине",
          kind: "boolean",
          required: false,
          visible: false,
          read_only: false,
          system: true,
        },
        {
          id: "project_id",
          label: "Project ID",
          kind: "text",
          required: false,
          visible: false,
          read_only: false,
          system: true,
        },
        {
          id: "heading_id",
          label: "Heading ID",
          kind: "text",
          required: false,
          visible: false,
          read_only: false,
          system: true,
        },
        {
          id: "area_id",
          label: "Area ID",
          kind: "text",
          required: false,
          visible: false,
          read_only: false,
          system: true,
        },
        {
          id: "tag_ids",
          label: "Tag IDs",
          kind: "text",
          required: false,
          visible: false,
          read_only: false,
          system: true,
        },
        {
          id: "checklist_items",
          label: "Checklist",
          kind: "text",
          required: false,
          visible: false,
          read_only: false,
          system: true,
        },
      ],
    }),
    uiSchemaJson: JSON.stringify({
      featured_fields: ["description"],
      visible_fields: [
        "description",
        "priority",
        "scheduled_date",
        "deadline",
        "reminder_date",
        "is_today",
        "is_evening",
        "is_completed",
        "is_cancelled",
      ],
      hidden_fields: [
        "created_at",
        "updated_at",
        "deleted_at",
        "is_trashed",
        "project_id",
        "heading_id",
        "area_id",
        "tag_ids",
        "checklist_items",
      ],
      read_only_fields: [],
      field_order: [
        "description",
        "priority",
        "scheduled_date",
        "deadline",
        "reminder_date",
        "is_today",
        "is_evening",
        "is_completed",
        "is_cancelled",
      ],
      header_layout: "inline",
      default_layout: "page",
      default_template_id: null,
    }),
    createdAt: now,
    updatedAt: now,
    systemLocked: false,
  };
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
      checklist_items: todo.checklistItems,
      recurrence_rule: todo.recurrenceRule ?? null,
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
  const props = object.propsJson ?? {};
  const description =
    asNullableString(props.description) ??
    extractPlainTextFromContentJson(object.contentJson);

  return {
    id: object.id.toLowerCase(),
    title: object.title,
    notes: description,
    priority: asNumber(props.priority, 0),
    scheduledDate: asNullableString(props.scheduled_date),
    deadline: asNullableString(props.deadline),
    reminderDate: asNullableString(props.reminder_date),
    isToday: asBoolean(props.is_today, false),
    isEvening: asBoolean(props.is_evening, false),
    isSomeday: asBoolean(props.is_someday, false),
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
    checklistItems: asChecklistItems(props.checklist_items),
    recurrenceRule: asRecurrenceRule(props.recurrence_rule),
  };
}
