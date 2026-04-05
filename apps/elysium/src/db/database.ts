import * as SQLite from "expo-sqlite";

const DB_NAME = "elysium.db";

let _db: SQLite.SQLiteDatabase | null = null;

export function getDb(): SQLite.SQLiteDatabase {
  if (!_db) {
    _db = SQLite.openDatabaseSync(DB_NAME);

    _db.execSync("PRAGMA journal_mode = WAL");

    _db.execSync("PRAGMA foreign_keys = ON");

    initSchema(_db);
  }

  return _db;
}

function initSchema(db: SQLite.SQLiteDatabase) {
  db.execSync(`
    CREATE TABLE IF NOT EXISTS nutrition_entries (
      id TEXT PRIMARY KEY,
      date TEXT NOT NULL,
      food_json TEXT NOT NULL,
      quantity REAL NOT NULL,
      meal_type TEXT NOT NULL,
      logged_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_nutrition_date ON nutrition_entries(date);

    CREATE TABLE IF NOT EXISTS water_entries (
      id TEXT PRIMARY KEY,
      date TEXT NOT NULL,
      amount INTEGER NOT NULL,
      time TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_water_date ON water_entries(date);

    CREATE TABLE IF NOT EXISTS custom_foods (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      brand TEXT,
      serving_size REAL NOT NULL,
      serving_unit TEXT NOT NULL,
      macros_json TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS recent_foods (
      id TEXT PRIMARY KEY,
      food_json TEXT NOT NULL,
      used_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    );
  `);
}

// ── Settings helpers ──

export function getSetting(key: string, fallback: string): string {
  const db = getDb();

  const row = db.getFirstSync<{ value: string }>(
    "SELECT value FROM settings WHERE key = ?",
    [key],
  );

  return row?.value ?? fallback;
}

export function setSetting(key: string, value: string) {
  const db = getDb();

  db.runSync("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)", [
    key,
    value,
  ]);
}

// ── Nutrition helpers ──

export interface NutritionRow {
  id: string;

  date: string;

  food_json: string;

  quantity: number;

  meal_type: string;

  logged_at: string;
}

export function loadAllNutritionEntries(): NutritionRow[] {
  return getDb().getAllSync<NutritionRow>(
    "SELECT * FROM nutrition_entries ORDER BY logged_at",
  );
}

export function insertNutritionEntry(row: NutritionRow) {
  getDb().runSync(
    "INSERT INTO nutrition_entries (id, date, food_json, quantity, meal_type, logged_at) VALUES (?, ?, ?, ?, ?, ?)",

    [
      row.id,
      row.date,
      row.food_json,
      row.quantity,
      row.meal_type,
      row.logged_at,
    ],
  );
}

export function updateNutritionQuantity(id: string, quantity: number) {
  getDb().runSync("UPDATE nutrition_entries SET quantity = ? WHERE id = ?", [
    quantity,
    id,
  ]);
}

export function deleteNutritionEntry(id: string) {
  getDb().runSync("DELETE FROM nutrition_entries WHERE id = ?", [id]);
}

// ── Water helpers ──

export interface WaterRow {
  id: string;

  date: string;

  amount: number;

  time: string;
}

export function loadAllWaterEntries(): WaterRow[] {
  return getDb().getAllSync<WaterRow>(
    "SELECT * FROM water_entries ORDER BY time",
  );
}

export function insertWaterEntry(row: WaterRow) {
  getDb().runSync(
    "INSERT INTO water_entries (id, date, amount, time) VALUES (?, ?, ?, ?)",

    [row.id, row.date, row.amount, row.time],
  );
}

export function deleteWaterEntry(id: string) {
  getDb().runSync("DELETE FROM water_entries WHERE id = ?", [id]);
}

// ── Food helpers ──

export interface CustomFoodRow {
  id: string;

  name: string;

  brand: string | null;

  serving_size: number;

  serving_unit: string;

  macros_json: string;
}

export function loadCustomFoods(): CustomFoodRow[] {
  return getDb().getAllSync<CustomFoodRow>(
    "SELECT * FROM custom_foods ORDER BY name",
  );
}

export function insertCustomFood(row: CustomFoodRow) {
  getDb().runSync(
    "INSERT OR IGNORE INTO custom_foods (id, name, brand, serving_size, serving_unit, macros_json) VALUES (?, ?, ?, ?, ?, ?)",

    [
      row.id,
      row.name,
      row.brand,
      row.serving_size,
      row.serving_unit,
      row.macros_json,
    ],
  );
}

export interface RecentFoodRow {
  id: string;

  food_json: string;

  used_at: string;
}

export function loadRecentFoods(): RecentFoodRow[] {
  return getDb().getAllSync<RecentFoodRow>(
    "SELECT * FROM recent_foods ORDER BY used_at DESC LIMIT 30",
  );
}

export function upsertRecentFood(row: RecentFoodRow) {
  getDb().runSync("DELETE FROM recent_foods WHERE id = ?", [row.id]);

  getDb().runSync(
    "INSERT INTO recent_foods (id, food_json, used_at) VALUES (?, ?, ?)",

    [row.id, row.food_json, row.used_at],
  );

  // Trim to 30

  getDb().runSync(
    "DELETE FROM recent_foods WHERE id NOT IN (SELECT id FROM recent_foods ORDER BY used_at DESC LIMIT 30)",
  );
}
