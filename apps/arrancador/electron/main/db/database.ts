import { execute, queryAll } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import { enableForeignKeys } from "./sqlite";

const CREATE_GAMES_TABLE_SQL = `
CREATE TABLE IF NOT EXISTS games (
  id TEXT PRIMARY KEY,
  ark_object_id TEXT,
  name TEXT NOT NULL,
  exe_path TEXT NOT NULL UNIQUE,
  exe_name TEXT NOT NULL,
  rawg_id INTEGER,
  description TEXT,
  released TEXT,
  background_image TEXT,
  metacritic INTEGER,
  rating REAL,
  genres TEXT,
  platforms TEXT,
  developers TEXT,
  publishers TEXT,
  cover_image TEXT,
  icon_image TEXT,
  is_favorite INTEGER DEFAULT 0,
  play_count INTEGER DEFAULT 0,
  total_playtime INTEGER DEFAULT 0,
  last_played TEXT,
  date_added TEXT NOT NULL,
  backup_enabled INTEGER DEFAULT 0,
  last_backup TEXT,
  backup_count INTEGER DEFAULT 0,
  save_path TEXT,
  save_path_checked INTEGER DEFAULT 0,
  user_rating INTEGER,
  user_note TEXT,
  play_status TEXT NOT NULL DEFAULT 'not_started'
)`;

const CREATE_PLAYTIME_DAILY_SQL = `
CREATE TABLE IF NOT EXISTS playtime_daily (
  game_id TEXT NOT NULL,
  date TEXT NOT NULL,
  seconds INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
  UNIQUE(game_id, date)
)`;

const CREATE_BACKUPS_SQL = `
CREATE TABLE IF NOT EXISTS backups (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL,
  backup_path TEXT NOT NULL,
  backup_size INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  is_auto INTEGER DEFAULT 0,
  notes TEXT,
  FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
)`;

const CREATE_SETTINGS_SQL = `
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
)`;

const CREATE_SCAN_DIRECTORIES_SQL = `
CREATE TABLE IF NOT EXISTS scan_directories (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL UNIQUE,
  last_scanned TEXT,
  auto_scan INTEGER DEFAULT 0
)`;

const CREATE_NOTIFICATIONS_SQL = `
CREATE TABLE IF NOT EXISTS notifications (
  id TEXT PRIMARY KEY,
  level TEXT NOT NULL,
  title TEXT NOT NULL,
  message TEXT NOT NULL,
  source TEXT,
  created_at TEXT NOT NULL,
  read_at TEXT
)`;

const CREATE_ACHIEVEMENTS_SQL = `
CREATE TABLE IF NOT EXISTS achievements (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  event_trigger TEXT NOT NULL,
  progress INTEGER NOT NULL DEFAULT 0,
  target INTEGER NOT NULL,
  unlocked INTEGER NOT NULL DEFAULT 0,
  unlocked_at TEXT,
  created_at TEXT NOT NULL
)`;

const CREATE_CATALOGUE_ITEMS_SQL = `
CREATE TABLE IF NOT EXISTS catalogue_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  rawg_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  payload TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'rawg',
  updated_at TEXT NOT NULL,
  UNIQUE(rawg_id, source)
)`;

const CREATE_GAME_PROCESS_BINDINGS_SQL = `
CREATE TABLE IF NOT EXISTS game_process_bindings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  game_id TEXT NOT NULL,
  match_type TEXT NOT NULL,
  match_value TEXT NOT NULL,
  normalized_value TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
  UNIQUE(match_type, normalized_value)
)`;

const GAME_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_games_name ON games(name)",
  "CREATE INDEX IF NOT EXISTS idx_games_favorite_name ON games(is_favorite, name)",
  "CREATE INDEX IF NOT EXISTS idx_games_ark_object_id ON games(ark_object_id)",
];

const PLAYTIME_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_playtime_daily_date ON playtime_daily(date)",
  "CREATE INDEX IF NOT EXISTS idx_playtime_daily_game ON playtime_daily(game_id)",
];

const BACKUP_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_backups_game_created ON backups(game_id, created_at DESC)",
];

const NOTIFICATION_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at DESC)",
];

const ACHIEVEMENT_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_achievements_trigger ON achievements(event_trigger)",
  "CREATE INDEX IF NOT EXISTS idx_achievements_unlocked ON achievements(unlocked, unlocked_at DESC)",
];

const CATALOGUE_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_catalogue_items_rawg_id ON catalogue_items(rawg_id)",
];

const GAME_PROCESS_BINDING_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_game_process_bindings_game_id ON game_process_bindings(game_id)",
  "CREATE INDEX IF NOT EXISTS idx_game_process_bindings_match ON game_process_bindings(match_type, normalized_value)",
];

const DEFAULT_SETTINGS: ReadonlyArray<[string, string]> = [
  ["ludusavi_path", ""],
  ["backup_directory", ""],
  ["auto_backup", "true"],
  ["backup_before_launch", "false"],
  ["backup_compression_enabled", "true"],
  ["backup_compression_level", "60"],
  ["backup_skip_compression_once", "false"],
  ["max_backups_per_game", "5"],
  ["theme", "system"],
  ["start_minimized_in_tray", "false"],
  ["rawg_api_key", ""],
];

async function getTableColumns(db: DbLike, tableName: string): Promise<Set<string>> {
  const rows = await queryAll<{ name: string }>(db, `PRAGMA table_info(${tableName})`);
  return new Set(rows.map((row) => row.name));
}

async function addColumnIfMissing(
  db: DbLike,
  columns: Set<string>,
  name: string,
  sql: string,
): Promise<void> {
  if (!columns.has(name)) {
    await execute(db, sql);
    columns.add(name);
  }
}

async function ensureIndexes(db: DbLike, statements: readonly string[]): Promise<void> {
  for (const sql of statements) {
    await execute(db, sql);
  }
}

async function ensureDefaultSettings(db: DbLike): Promise<void> {
  for (const [key, value] of DEFAULT_SETTINGS) {
    await execute(
      db,
      "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
      [key, value],
    );
  }
}

export async function ensureGameColumns(db: DbLike): Promise<void> {
  const columns = await getTableColumns(db, "games");

  await addColumnIfMissing(db, columns, "ark_object_id", "ALTER TABLE games ADD COLUMN ark_object_id TEXT");
  await addColumnIfMissing(db, columns, "user_rating", "ALTER TABLE games ADD COLUMN user_rating INTEGER");
  await addColumnIfMissing(db, columns, "user_note", "ALTER TABLE games ADD COLUMN user_note TEXT");
  await addColumnIfMissing(db, columns, "save_path", "ALTER TABLE games ADD COLUMN save_path TEXT");
  await addColumnIfMissing(
    db,
    columns,
    "save_path_checked",
    "ALTER TABLE games ADD COLUMN save_path_checked INTEGER DEFAULT 0",
  );
  await addColumnIfMissing(db, columns, "icon_image", "ALTER TABLE games ADD COLUMN icon_image TEXT");
  await addColumnIfMissing(
    db,
    columns,
    "play_status",
    "ALTER TABLE games ADD COLUMN play_status TEXT NOT NULL DEFAULT 'not_started'",
  );
}

export async function initAppDatabase(db: DbLike): Promise<void> {
  await enableForeignKeys(db);
  await execute(db, CREATE_GAMES_TABLE_SQL);
  await ensureGameColumns(db);
  await ensureIndexes(db, GAME_INDEXES);

  await execute(db, CREATE_PLAYTIME_DAILY_SQL);
  await ensureIndexes(db, PLAYTIME_INDEXES);

  await execute(db, CREATE_BACKUPS_SQL);
  await ensureIndexes(db, BACKUP_INDEXES);

  await execute(db, CREATE_SETTINGS_SQL);
  await execute(db, CREATE_SCAN_DIRECTORIES_SQL);

  await execute(db, CREATE_NOTIFICATIONS_SQL);
  await ensureIndexes(db, NOTIFICATION_INDEXES);

  await execute(db, CREATE_ACHIEVEMENTS_SQL);
  await ensureIndexes(db, ACHIEVEMENT_INDEXES);

  await execute(db, CREATE_CATALOGUE_ITEMS_SQL);
  await ensureIndexes(db, CATALOGUE_INDEXES);

  await execute(db, CREATE_GAME_PROCESS_BINDINGS_SQL);
  await ensureIndexes(db, GAME_PROCESS_BINDING_INDEXES);

  await ensureDefaultSettings(db);
}

export async function initGameDatabase(db: DbLike): Promise<void> {
  await initAppDatabase(db);
}

export interface GameDatabaseInitOptions {
  initialize?: boolean;
}

export async function openGameDatabase(
  db: DbLike,
  options: GameDatabaseInitOptions = {},
): Promise<DbLike> {
  if (options.initialize !== false) {
    await initAppDatabase(db);
  }
  return db;
}
