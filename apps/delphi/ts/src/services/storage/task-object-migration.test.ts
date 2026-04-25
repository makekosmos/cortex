import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { Priority, type TodoItem } from "@/types/task";
import {
  DELPHI_TASK_OBJECT_MIGRATION_KEY,
  DELPHI_TASK_OBJECT_MIGRATION_REPORT_KEY,
  batchUpsertTodosObjectFirst,
  listDelphiTaskObjectsAsTodos,
  loadAllObjectFirst,
  migrateLegacyTodosToTaskObjects,
  upsertTodoObjectFirst,
  type DelphiTaskObjectMigrationDeps,
  type LegacyTaskData,
} from "../../../shared/task-object-migration";
import {
  DELPHI_TASK_OBJECT_TYPE_ID,
  todoToArkTaskObject,
  type ArkObjectRecord,
  type ArkObjectTypeRecord,
} from "../../../shared/task-ark";

function makeTodo(id: string, title = `Task ${id}`): TodoItem {
  return {
    id,
    title,
    notes: `Notes for ${id}`,
    priority: Priority.Medium,
    scheduledDate: null,
    deadline: null,
    reminderDate: null,
    isToday: false,
    isEvening: false,
    isSomeday: false,
    isCompleted: false,
    completedAt: null,
    isCancelled: false,
    cancelledAt: null,
    isTrashed: false,
    sortOrder: 0,
    createdAt: "2026-04-25T00:00:00.000Z",
    headingId: null,
    projectId: null,
    areaId: null,
    tagIds: [],
    checklistItems: [],
    recurrenceRule: null,
  };
}

function createDeps(
  legacyTodos: TodoItem[],
  initialObjects: ArkObjectRecord[] = [],
  options: { failObjectIds?: string[] } = {},
) {
  const syncKv = new Map<string, string>();
  const objects = new Map(initialObjects.map((object) => [object.id, object]));
  const objectTypes = new Map<string, ArkObjectTypeRecord>();
  const upsertedObjectIds: string[] = [];
  const failObjectIds = new Set(options.failObjectIds ?? []);
  const legacyData: LegacyTaskData = {
    todos: legacyTodos,
    projects: [],
    areas: [],
    tags: [],
    headings: [],
  };

  const deps: DelphiTaskObjectMigrationDeps = {
    async loadAll() {
      return legacyData;
    },
    async getSyncKv(key) {
      return syncKv.get(key) ?? null;
    },
    async setSyncKv(key, value) {
      syncKv.set(key, value);
      return true;
    },
    async listObjects() {
      return Array.from(objects.values());
    },
    async upsertObject(object) {
      if (failObjectIds.has(object.id)) {
        throw new Error(`Cannot migrate ${object.id}`);
      }
      objects.set(object.id, object);
      upsertedObjectIds.push(object.id);
      return true;
    },
    async upsertObjectType(objectType) {
      objectTypes.set(objectType.id, objectType);
      return true;
    },
    async deleteObject(id) {
      return objects.delete(id);
    },
  };

  return { deps, syncKv, objects, objectTypes, upsertedObjectIds };
}

describe("Delphi task object migration", () => {
  it("writes a backup and structured success report for migrated todos", async () => {
    const backupDir = fs.mkdtempSync(path.join(os.tmpdir(), "delphi-task-migration-"));
    const todos = [makeTodo("todo-a"), makeTodo("todo-b")];
    const { deps, syncKv } = createDeps(todos);

    const report = await migrateLegacyTodosToTaskObjects(deps, {
      backupDir,
      now: () => "2026-04-25T12:00:00.000Z",
    });

    expect(report.status).toBe("success");
    expect(report.sourceCount).toBe(2);
    expect(report.migratedCount).toBe(2);
    expect(report.failedCount).toBe(0);
    expect(report.backupPath).toBeTruthy();
    expect(fs.existsSync(report.backupPath!)).toBe(true);
    expect(JSON.parse(fs.readFileSync(report.backupPath!, "utf8")).legacyTodos).toHaveLength(2);
    expect(JSON.parse(syncKv.get(DELPHI_TASK_OBJECT_MIGRATION_REPORT_KEY) ?? "{}").status).toBe("success");
  });

  it("migrates legacy todos into task_obj objects and writes a marker", async () => {
    const todos = [makeTodo("todo-a"), makeTodo("todo-b")];
    const { deps, syncKv, objects, objectTypes, upsertedObjectIds } = createDeps(todos);

    await migrateLegacyTodosToTaskObjects(deps, () => "2026-04-25T12:00:00.000Z");

    expect(objectTypes.has(DELPHI_TASK_OBJECT_TYPE_ID)).toBe(true);
    expect(upsertedObjectIds).toEqual(["todo-a", "todo-b"]);
    expect(objects.get("todo-a")?.typeId).toBe(DELPHI_TASK_OBJECT_TYPE_ID);
    expect(objects.get("todo-b")?.title).toBe("Task todo-b");
    expect(syncKv.get(DELPHI_TASK_OBJECT_MIGRATION_KEY)).toBe("2026-04-25T12:00:00.000Z");
  });

  it("continues after record failures and does not write success marker", async () => {
    const todos = [makeTodo("todo-a"), makeTodo("todo-b"), makeTodo("todo-c")];
    const { deps, syncKv, objects } = createDeps(todos, [], { failObjectIds: ["todo-b"] });

    const report = await migrateLegacyTodosToTaskObjects(deps, {
      now: () => "2026-04-25T12:00:00.000Z",
    });

    expect(report.status).toBe("partial_failure");
    expect(report.migratedCount).toBe(2);
    expect(report.failedCount).toBe(1);
    expect(report.errors).toEqual([{ id: "todo-b", message: "Cannot migrate todo-b" }]);
    expect(objects.has("todo-a")).toBe(true);
    expect(objects.has("todo-c")).toBe(true);
    expect(syncKv.has(DELPHI_TASK_OBJECT_MIGRATION_KEY)).toBe(false);
    expect(JSON.parse(syncKv.get(DELPHI_TASK_OBJECT_MIGRATION_REPORT_KEY) ?? "{}").status).toBe("partial_failure");
  });

  it("is idempotent and does not duplicate already migrated task objects", async () => {
    const existingTodo = makeTodo("todo-a");
    const newTodo = makeTodo("todo-b");
    const { deps, objects, upsertedObjectIds } = createDeps(
      [existingTodo, newTodo],
      [todoToArkTaskObject(existingTodo)],
    );

    await migrateLegacyTodosToTaskObjects(deps, () => "first");
    await migrateLegacyTodosToTaskObjects(deps, () => "second");

    expect(upsertedObjectIds).toEqual(["todo-b"]);
    expect(objects.size).toBe(2);
  });

  it("returns object-first todos from loadAll after migration", async () => {
    const legacyTodo = makeTodo("todo-a", "Legacy title");
    const objectTodo = makeTodo("todo-a", "Object title");
    const { deps } = createDeps([legacyTodo], [todoToArkTaskObject(objectTodo)]);

    const result = await loadAllObjectFirst(deps);

    expect(result.todos).toHaveLength(1);
    expect(result.todos[0]?.title).toBe("Object title");
  });

  it("writes single and batch task mutations as ARK objects", async () => {
    const single = makeTodo("single");
    const batch = [makeTodo("batch-a"), makeTodo("batch-b")];
    const { deps, objects } = createDeps([]);

    await upsertTodoObjectFirst(deps, single);
    await batchUpsertTodosObjectFirst(deps, batch);
    const todos = await listDelphiTaskObjectsAsTodos(deps);

    expect(objects.get("single")?.typeId).toBe(DELPHI_TASK_OBJECT_TYPE_ID);
    expect(todos.map((todo) => todo.id).sort()).toEqual(["batch-a", "batch-b", "single"]);
  });
});
