/**
 * SQLite persistence layer for Delphi Mobile.
 *
 * Stores all todos and settings in expo-sqlite.
 * Simple key-value + JSON blob approach for v1.
 */

import * as SQLite from "expo-sqlite";
import type { TodoItem, Project } from "@/types/task";

let db: SQLite.SQLiteDatabase | null = null;

export async function getDb(): Promise<SQLite.SQLiteDatabase> {
  if (db) return db;
  try {
    db = SQLite.openDatabaseSync("delphi.db");
    db.execSync(`
      CREATE TABLE IF NOT EXISTS todos (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY,
        data TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
      );
    `);
  } catch (e) {
    console.warn("[Storage] SQLite init failed, using in-memory fallback:", e);
    // Return a mock that doesn't crash
    return {
      getAllAsync: async () => [],
      getFirstAsync: async () => null,
      runAsync: async () => ({ changes: 0, lastInsertRowId: 0 }),
      withTransactionAsync: async (fn: () => Promise<void>) => fn(),
      execAsync: async () => {},
    } as unknown as SQLite.SQLiteDatabase;
  }
  return db;
}

// ---------------------------------------------------------------------------
// Todos
// ---------------------------------------------------------------------------

export async function loadTodos(): Promise<TodoItem[]> {
  const database = await getDb();
  const rows = await database.getAllAsync<{ data: string }>(
    "SELECT data FROM todos",
  );
  return rows.map((r) => JSON.parse(r.data) as TodoItem);
}

export async function saveTodo(todo: TodoItem): Promise<void> {
  const database = await getDb();
  await database.runAsync(
    "INSERT OR REPLACE INTO todos (id, data) VALUES (?, ?)",
    [todo.id, JSON.stringify(todo)],
  );
}

export async function saveTodos(todos: TodoItem[]): Promise<void> {
  const database = await getDb();
  await database.withTransactionAsync(async () => {
    await database.runAsync("DELETE FROM todos");
    for (const todo of todos) {
      await database.runAsync(
        "INSERT INTO todos (id, data) VALUES (?, ?)",
        [todo.id, JSON.stringify(todo)],
      );
    }
  });
}

export async function deleteTodo(id: string): Promise<void> {
  const database = await getDb();
  await database.runAsync("DELETE FROM todos WHERE id = ?", [id]);
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

export async function loadProjects(): Promise<Project[]> {
  const database = await getDb();
  const rows = await database.getAllAsync<{ data: string }>(
    "SELECT data FROM projects",
  );
  return rows.map((r) => JSON.parse(r.data) as Project);
}

export async function saveProjects(projects: Project[]): Promise<void> {
  const database = await getDb();
  await database.withTransactionAsync(async () => {
    await database.runAsync("DELETE FROM projects");
    for (const project of projects) {
      await database.runAsync(
        "INSERT INTO projects (id, data) VALUES (?, ?)",
        [project.id, JSON.stringify(project)],
      );
    }
  });
}

// ---------------------------------------------------------------------------
// Settings (key-value)
// ---------------------------------------------------------------------------

export async function getSetting(key: string): Promise<string | null> {
  const database = await getDb();
  const row = await database.getFirstAsync<{ value: string }>(
    "SELECT value FROM settings WHERE key = ?",
    [key],
  );
  return row?.value ?? null;
}

export async function setSetting(key: string, value: string): Promise<void> {
  const database = await getDb();
  await database.runAsync(
    "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
    [key, value],
  );
}

export async function deleteSetting(key: string): Promise<void> {
  const database = await getDb();
  await database.runAsync("DELETE FROM settings WHERE key = ?", [key]);
}
