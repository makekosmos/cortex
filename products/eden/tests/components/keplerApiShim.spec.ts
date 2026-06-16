import { afterEach, describe, expect, test, vi } from "vitest";
import {
  deleteEntry,
  ensureTaskObjectTypeRegistered,
  installKeplerApiShim,
  loadEntry,
  listEntries,
  listNoteTypes,
  softDeleteTask,
} from "@/lib/kepler-api-shim";

const previousKepler = window.kepler;
const previousApi = window.api;

afterEach(() => {
  vi.useRealTimers();
  window.localStorage.removeItem("eden-extension-visible-object-type-ids");
  Object.defineProperty(window, "kepler", {
    configurable: true,
    value: previousKepler,
  });
  Object.defineProperty(window, "api", {
    configurable: true,
    value: previousApi,
  });
});

function installArkMock(handler: (operation: string, params?: Record<string, unknown>) => unknown) {
  Object.defineProperty(window, "kepler", {
    configurable: true,
    value: {
      ark: {
        request: async (operation: string, params?: Record<string, unknown>) =>
          handler(operation, params),
        subscribe: () => () => undefined,
      },
    },
  });
}

describe("kepler-api-shim timestamps", () => {
  test("listEntries does not turn missing updatedAt into current time", async () => {
    const createdAt = "2024-01-02T03:04:05.000Z";
    const operations: string[] = [];
    const requests: Record<string, unknown> = {
      list_object_summaries: [
        {
          id: "old-note",
          typeId: "note_obj",
          title: "Старая заметка",
          propsJson: {},
          createdAt,
          updatedAt: null,
          deletedAt: null,
        },
      ],
    };

    installArkMock((operation) => {
      operations.push(operation);
      return requests[operation];
    });

    const [entry] = await listEntries();

    expect(entry.updated_at).toBe(Date.parse(createdAt));
    expect(entry.updated_at).toBe(entry.created_at);
    expect(entry.content_json).toBe(JSON.stringify({ type: "markdown", version: 1, text: "" }));
    expect(operations).toContain("list_object_summaries");
    expect(operations).not.toContain("list_objects");
  });

  test("listEntries hides soft-deleted objects", async () => {
    installArkMock((operation) => {
      const requests: Record<string, unknown> = {
        list_object_summaries: [
          {
            id: "active-note",
            typeId: "note_obj",
            title: "Active",
            propsJson: {},
            createdAt: "2024-01-02T03:04:05.000Z",
            updatedAt: "2024-01-02T03:04:05.000Z",
            deletedAt: null,
          },
          {
            id: "trashed-note",
            typeId: "note_obj",
            title: "Trashed",
            propsJson: {},
            createdAt: "2024-01-02T03:04:05.000Z",
            updatedAt: "2024-01-03T03:04:05.000Z",
            deletedAt: "2024-01-04T03:04:05.000Z",
          },
        ],
      };
      return requests[operation];
    });

    const entries = await listEntries();

    expect(entries.map((entry) => entry.id)).toEqual(["active-note"]);
  });

  test("listEntries uses summary query per visible object type", async () => {
    window.localStorage.setItem(
      "eden-extension-visible-object-type-ids",
      JSON.stringify(["note_obj", "book_obj"]),
    );
    const operations: Array<{ operation: string; params?: Record<string, unknown> }> = [];

    installArkMock((operation, params) => {
      operations.push({ operation, params });
      if (operation === "list_object_summaries_by_type") {
        return [
          {
            id: `${params?.type_id}-1`,
            typeId: params?.type_id,
            title: String(params?.type_id),
            propsJson: {},
            createdAt: "2024-01-02T03:04:05.000Z",
            updatedAt: "2024-01-02T03:04:05.000Z",
            deletedAt: null,
          },
        ];
      }
      throw new Error(`Unexpected operation: ${operation}`);
    });

    const entries = await listEntries();

    expect(entries.map((entry) => entry.type_id)).toEqual(["note_obj", "book_obj"]);
    expect(operations.map((call) => call.operation)).toEqual([
      "list_object_summaries_by_type",
      "list_object_summaries_by_type",
    ]);
    expect(operations[0]?.params).toEqual({ type_id: "note_obj" });
    expect(operations[1]?.params).toEqual({ type_id: "book_obj" });
  });

  test("listEntries falls back to full object list when summary query is unavailable", async () => {
    const operations: string[] = [];

    installArkMock((operation) => {
      operations.push(operation);
      if (operation === "list_object_summaries") {
        throw new Error("unknown operation");
      }
      if (operation === "list_objects") {
        return [
          {
            id: "note-1",
            typeId: "note_obj",
            title: "Fallback",
            contentJson: { type: "markdown", version: 1, text: "body" },
            propsJson: {},
            createdAt: "2024-01-02T03:04:05.000Z",
            updatedAt: "2024-01-02T03:04:05.000Z",
            deletedAt: null,
          },
        ];
      }
      throw new Error(`Unexpected operation: ${operation}`);
    });

    const [entry] = await listEntries();

    expect(entry.id).toBe("note-1");
    expect(entry.content_json).toBe(JSON.stringify({ type: "markdown", version: 1, text: "" }));
    expect(operations).toContain("list_object_summaries");
    expect(operations).toContain("list_objects");
  });

  test("listTrashEntries shows soft-deleted objects only", async () => {
    installArkMock((operation) => {
      const requests: Record<string, unknown> = {
        list_objects: [
          {
            id: "active-note",
            typeId: "note_obj",
            title: "Active",
            contentJson: { type: "markdown", version: 1, text: "" },
            propsJson: {},
            createdAt: "2024-01-02T03:04:05.000Z",
            updatedAt: "2024-01-02T03:04:05.000Z",
            deletedAt: null,
          },
          {
            id: "older-trash",
            typeId: "note_obj",
            title: "Older trash",
            contentJson: { type: "markdown", version: 1, text: "" },
            propsJson: {},
            createdAt: "2024-01-02T03:04:05.000Z",
            updatedAt: "2024-01-03T03:04:05.000Z",
            deletedAt: "2024-01-04T03:04:05.000Z",
          },
          {
            id: "newer-trash",
            typeId: "note_obj",
            title: "Newer trash",
            contentJson: { type: "markdown", version: 1, text: "" },
            propsJson: {},
            createdAt: "2024-01-02T03:04:05.000Z",
            updatedAt: "2024-01-03T03:04:05.000Z",
            deletedAt: "2024-01-05T03:04:05.000Z",
          },
        ],
        list_object_links: [],
      };
      return requests[operation];
    });
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: undefined,
    });
    installKeplerApiShim();

    const entries = await window.api.listTrashEntries();

    expect(entries.map((entry) => entry.id)).toEqual(["newer-trash", "older-trash"]);
    expect(entries.every((entry) => entry.deleted_at !== null)).toBe(true);
  });

  test("loadEntry does not open soft-deleted objects", async () => {
    installArkMock((operation) => {
      const requests: Record<string, unknown> = {
        get_object: {
          id: "trashed-note",
          typeId: "note_obj",
          title: "Trashed",
          contentJson: { type: "markdown", version: 1, text: "" },
          propsJson: {},
          createdAt: "2024-01-02T03:04:05.000Z",
          updatedAt: "2024-01-03T03:04:05.000Z",
          deletedAt: "2024-01-04T03:04:05.000Z",
        },
        list_object_links: [],
        list_object_types: [],
      };
      return requests[operation];
    });

    await expect(loadEntry("trashed-note")).resolves.toBeUndefined();
  });
});

describe("kepler-api-shim delete semantics", () => {
  test("deleteEntry soft-deletes through upsert_object instead of delete_object", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-05-06T07:08:09.000Z"));
    const calls: Array<{ operation: string; params?: Record<string, unknown> }> = [];
    const existing = {
      id: "note-1",
      typeId: "note_obj",
      title: "Note",
      contentJson: { type: "markdown", version: 1, text: "" },
      propsJson: {},
      createdAt: "2024-01-02T03:04:05.000Z",
      updatedAt: "2024-01-02T03:04:05.000Z",
      deletedAt: null,
    };

    installArkMock((operation, params) => {
      calls.push({ operation, params });
      if (operation === "get_object") return existing;
      if (operation === "upsert_object") return true;
      throw new Error(`Unexpected operation: ${operation}`);
    });

    await expect(deleteEntry("note-1")).resolves.toEqual({ ok: true, entryId: "note-1" });

    expect(calls.map((call) => call.operation)).toEqual(["get_object", "upsert_object"]);
    expect(calls).not.toContainEqual(expect.objectContaining({ operation: "delete_object" }));
    expect(calls[1]?.params?.object).toMatchObject({
      id: "note-1",
      updatedAt: "2024-05-06T07:08:09.000Z",
      deletedAt: "2024-05-06T07:08:09.000Z",
    });
  });

  test("softDeleteTask soft-deletes through upsert_object instead of delete_object", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2024-05-06T07:08:09.000Z"));
    const calls: Array<{ operation: string; params?: Record<string, unknown> }> = [];
    const existing = {
      id: "task-1",
      typeId: "task_obj",
      title: "Task",
      contentJson: { type: "markdown", version: 1, text: "" },
      propsJson: { status: "triage" },
      createdAt: "2024-01-02T03:04:05.000Z",
      updatedAt: "2024-01-02T03:04:05.000Z",
      deletedAt: null,
    };

    installArkMock((operation, params) => {
      calls.push({ operation, params });
      if (operation === "get_object") return existing;
      if (operation === "upsert_object") return true;
      throw new Error(`Unexpected operation: ${operation}`);
    });

    await softDeleteTask("task-1");

    expect(calls.map((call) => call.operation)).toEqual(["get_object", "upsert_object"]);
    expect(calls).not.toContainEqual(expect.objectContaining({ operation: "delete_object" }));
    expect(calls[1]?.params?.object).toMatchObject({
      id: "task-1",
      updatedAt: "2024-05-06T07:08:09.000Z",
      deletedAt: "2024-05-06T07:08:09.000Z",
    });
  });

  test("softDeleteTask leaves already trashed tasks untouched", async () => {
    const calls: string[] = [];

    installArkMock((operation) => {
      calls.push(operation);
      if (operation === "get_object") {
        return {
          id: "task-1",
          typeId: "task_obj",
          title: "Task",
          contentJson: { type: "markdown", version: 1, text: "" },
          propsJson: {},
          createdAt: "2024-01-02T03:04:05.000Z",
          updatedAt: "2024-01-02T03:04:05.000Z",
          deletedAt: "2024-01-04T03:04:05.000Z",
        };
      }
      throw new Error(`Unexpected operation: ${operation}`);
    });

    await softDeleteTask("task-1");

    expect(calls).toEqual(["get_object"]);
  });
});

describe("kepler-api-shim task object metadata", () => {
  test("normalizes legacy empty task_obj metadata before the editor renders it", async () => {
    installArkMock((operation) => {
      if (operation === "list_object_types") {
        return [
          {
            id: "task_obj",
            name: "Задача",
            schemaJson: "{}",
            uiSchemaJson: "{}",
            createdAt: "2024-01-02T03:04:05.000Z",
            updatedAt: "2024-01-02T03:04:05.000Z",
            systemLocked: false,
          },
        ];
      }
      throw new Error(`Unexpected operation: ${operation}`);
    });

    const [taskType] = await listNoteTypes();

    expect(JSON.parse(taskType.schema_json)).toMatchObject({
      fields: [{ id: "deadline", kind: "date" }],
    });
    expect(JSON.parse(taskType.ui_schema_json ?? "{}")).toMatchObject({
      featured_fields: ["deadline"],
      visible_fields: ["deadline"],
    });
  });

  test("registers task_obj with an editable deadline field for Eden object pages", async () => {
    const calls: Array<{ operation: string; params?: Record<string, unknown> }> = [];

    installArkMock((operation, params) => {
      calls.push({ operation, params });
      if (operation === "upsert_object_type") return true;
      throw new Error(`Unexpected operation: ${operation}`);
    });

    await ensureTaskObjectTypeRegistered();

    const objectType = calls[0]?.params?.object_type as
      | { schemaJson?: string; uiSchemaJson?: string }
      | undefined;
    expect(calls.map((call) => call.operation)).toEqual(["upsert_object_type"]);
    expect(JSON.parse(objectType?.schemaJson ?? "{}")).toMatchObject({
      fields: [
        {
          id: "deadline",
          label: "Дедлайн",
          kind: "date",
          visible: true,
          read_only: false,
        },
      ],
    });
    expect(JSON.parse(objectType?.uiSchemaJson ?? "{}")).toMatchObject({
      featured_fields: ["deadline"],
      visible_fields: ["deadline"],
      field_order: ["deadline"],
      default_layout: "page",
      collection_name: "Задачи",
    });
  });
});
