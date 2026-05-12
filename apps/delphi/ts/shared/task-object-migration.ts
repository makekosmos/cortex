import fs from "node:fs/promises";
import path from "node:path";

import type { TodoItem } from "../src/types/task";
import {
  DELPHI_TASK_OBJECT_TYPE_ID,
  arkTaskObjectToTodo,
  createDelphiTaskObjectTypeRecord,
  todoToArkTaskObject,
  type ArkObjectRecord,
  type ArkObjectTypeRecord,
} from "./task-ark";

export const DELPHI_TASK_OBJECT_MIGRATION_KEY = "delphi.task_obj_migration.v1";
export const DELPHI_TASK_OBJECT_MIGRATION_REPORT_KEY = `${DELPHI_TASK_OBJECT_MIGRATION_KEY}.report`;
const DELPHI_TASK_OBJECT_MIGRATION_ID = "delphi-task-objects-v1";

export interface LegacyTaskData {
  todos: TodoItem[];
  [key: string]: unknown;
}

export interface DelphiTaskObjectMigrationDeps {
  loadAll(): Promise<LegacyTaskData>;
  getSyncKv(key: string): Promise<string | null>;
  setSyncKv(key: string, value: string): Promise<boolean>;
  listObjects(): Promise<ArkObjectRecord[]>;
  upsertObject(object: ArkObjectRecord): Promise<boolean>;
  upsertObjectType(objectType: ArkObjectTypeRecord): Promise<boolean>;
  deleteObject(id: string): Promise<boolean>;
}

export interface MigrationRecordError {
  id: string;
  message: string;
}

export interface DelphiTaskMigrationReport {
  migrationId: string;
  startedAt: string;
  finishedAt: string;
  status: "success" | "partial_failure" | "failed" | "noop";
  sourceCount: number;
  migratedCount: number;
  skippedCount: number;
  failedCount: number;
  backupPath: string | null;
  errors: MigrationRecordError[];
}

export interface DelphiTaskMigrationOptions {
  backupDir?: string | null;
  now?: () => string;
}

async function writeMigrationBackup(
  backupDir: string | null | undefined,
  snapshot: unknown,
): Promise<string | null> {
  if (!backupDir) {
    return null;
  }

  await fs.mkdir(backupDir, { recursive: true });
  const backupPath = path.join(backupDir, `${DELPHI_TASK_OBJECT_MIGRATION_ID}.json`);
  try {
    await fs.writeFile(backupPath, JSON.stringify(snapshot, null, 2), { flag: "wx" });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") {
      throw error;
    }
  }
  return backupPath;
}

function reportStatus(report: Pick<DelphiTaskMigrationReport, "sourceCount" | "migratedCount" | "failedCount">) {
  if (report.sourceCount === 0) {
    return "noop" as const;
  }
  if (report.failedCount === 0) {
    return "success" as const;
  }
  if (report.migratedCount > 0) {
    return "partial_failure" as const;
  }
  return "failed" as const;
}

async function persistReport(
  deps: Pick<DelphiTaskObjectMigrationDeps, "setSyncKv">,
  report: DelphiTaskMigrationReport,
) {
  await deps.setSyncKv(DELPHI_TASK_OBJECT_MIGRATION_REPORT_KEY, JSON.stringify(report));
}

export async function ensureDelphiTaskObjectType(
  deps: Pick<DelphiTaskObjectMigrationDeps, "upsertObjectType">,
): Promise<void> {
  await deps.upsertObjectType(createDelphiTaskObjectTypeRecord());
}

export async function listDelphiTaskObjectsAsTodos(
  deps: Pick<DelphiTaskObjectMigrationDeps, "listObjects">,
): Promise<TodoItem[]> {
  const objects = await deps.listObjects();
  return objects
    .filter((object) => object.typeId === DELPHI_TASK_OBJECT_TYPE_ID)
    .map(arkTaskObjectToTodo);
}

export async function migrateLegacyTodosToTaskObjects(
  deps: DelphiTaskObjectMigrationDeps,
  optionsOrNow: DelphiTaskMigrationOptions | (() => string) = {},
): Promise<DelphiTaskMigrationReport> {
  const options: DelphiTaskMigrationOptions =
    typeof optionsOrNow === "function" ? { now: optionsOrNow } : optionsOrNow;
  const now = options.now ?? (() => new Date().toISOString());
  const startedAt = now();
  const migrationDone = await deps.getSyncKv(DELPHI_TASK_OBJECT_MIGRATION_KEY);
  const legacyData = await deps.loadAll();
  const backupPath = await writeMigrationBackup(
    options.backupDir,
    {
      migrationId: DELPHI_TASK_OBJECT_MIGRATION_ID,
      createdAt: startedAt,
      legacyTodos: legacyData.todos,
    },
  );
  const report: DelphiTaskMigrationReport = {
    migrationId: DELPHI_TASK_OBJECT_MIGRATION_ID,
    startedAt,
    finishedAt: startedAt,
    status: "noop",
    sourceCount: legacyData.todos.length,
    migratedCount: 0,
    skippedCount: 0,
    failedCount: 0,
    backupPath,
    errors: [],
  };

  if (legacyData.todos.length === 0) {
    if (!migrationDone) {
      await deps.setSyncKv(DELPHI_TASK_OBJECT_MIGRATION_KEY, now());
    }
    report.finishedAt = now();
    report.status = "noop";
    await persistReport(deps, report);
    return report;
  }

  try {
    await ensureDelphiTaskObjectType(deps);
  } catch (error) {
    report.failedCount = legacyData.todos.length;
    report.errors.push({
      id: DELPHI_TASK_OBJECT_TYPE_ID,
      message: (error as Error).message,
    });
    report.finishedAt = now();
    report.status = "failed";
    await persistReport(deps, report);
    return report;
  }

  const existing = await deps.listObjects();
  const existingTaskIds = new Set(
    existing
      .filter((object) => object.typeId === DELPHI_TASK_OBJECT_TYPE_ID)
      .map((object) => object.id),
  );

  for (const todo of legacyData.todos) {
    if (existingTaskIds.has(todo.id)) {
      report.skippedCount += 1;
      continue;
    }

    try {
      await deps.upsertObject(todoToArkTaskObject(todo));
      existingTaskIds.add(todo.id);
      report.migratedCount += 1;
    } catch (error) {
      report.failedCount += 1;
      report.errors.push({
        id: todo.id,
        message: (error as Error).message,
      });
    }
  }

  report.finishedAt = now();
  report.status = reportStatus(report);
  await persistReport(deps, report);
  if (report.failedCount === 0) {
    await deps.setSyncKv(DELPHI_TASK_OBJECT_MIGRATION_KEY, report.finishedAt);
  }
  return report;
}

export async function loadAllObjectFirst(
  deps: DelphiTaskObjectMigrationDeps,
  options?: DelphiTaskMigrationOptions,
): Promise<LegacyTaskData> {
  const legacyData = await deps.loadAll();
  await migrateLegacyTodosToTaskObjects(deps, options ?? {});
  const objectTodos = await listDelphiTaskObjectsAsTodos(deps);
  return {
    ...legacyData,
    todos: objectTodos,
  };
}

export async function upsertTodoObjectFirst(
  deps: Pick<DelphiTaskObjectMigrationDeps, "upsertObject" | "upsertObjectType">,
  todo: TodoItem,
): Promise<boolean> {
  await ensureDelphiTaskObjectType(deps);
  await deps.upsertObject(todoToArkTaskObject(todo));
  return true;
}

export async function batchUpsertTodosObjectFirst(
  deps: Pick<DelphiTaskObjectMigrationDeps, "upsertObject" | "upsertObjectType">,
  todos: TodoItem[],
): Promise<boolean> {
  if (todos.length === 0) {
    return true;
  }

  await ensureDelphiTaskObjectType(deps);
  for (const todo of todos) {
    await deps.upsertObject(todoToArkTaskObject(todo));
  }
  return true;
}
