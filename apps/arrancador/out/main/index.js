// electron/main/index.ts
import { mkdir as mkdir2 } from "node:fs/promises";
import path18 from "node:path";
import { fileURLToPath } from "node:url";
import { app as app3 } from "electron";

// electron/main/backend.ts
import path15 from "node:path";
import {
  app as app2,
  BrowserWindow,
  dialog,
  ipcMain,
  shell
} from "electron";

// electron/main/helpers/db.ts
async function runInTransaction(db, fn) {
  if (db.transaction) {
    return await Promise.resolve(db.transaction(fn));
  }
  return await Promise.resolve(fn(db));
}
async function queryAll(db, sql, params = []) {
  return await Promise.resolve(db.all(sql, params));
}
async function queryOne(db, sql, params = []) {
  return await Promise.resolve(db.get(sql, params));
}
async function execute(db, sql, params = []) {
  const result = await Promise.resolve(db.run(sql, params));
  return result.changes;
}

// electron/main/db/sqlite.ts
import { createRequire } from "node:module";
function openSqliteDatabase(filePath, options = {}) {
  const require2 = createRequire(import.meta.url);
  const BetterSqlite3 = require2("better-sqlite3");
  const nativeDb = new BetterSqlite3(filePath, {
    ...typeof options.readonly === "boolean" ? { readonly: options.readonly } : {},
    ...typeof options.fileMustExist === "boolean" ? { fileMustExist: options.fileMustExist } : {},
    ...typeof options.timeoutMs === "number" ? { timeout: options.timeoutMs } : {},
    ...typeof options.verbose === "function" ? { verbose: options.verbose } : {}
  });
  const prepare = (sql) => nativeDb.prepare(sql.replace(/\?\d+/g, "?"));
  let wrapper;
  wrapper = {
    all(sql, params = []) {
      const statement = prepare(sql);
      return params.length === 0 ? statement.all() : statement.all(params);
    },
    get(sql, params = []) {
      const statement = prepare(sql);
      return params.length === 0 ? statement.get() : statement.get(params);
    },
    run(sql, params = []) {
      const statement = prepare(sql);
      return params.length === 0 ? statement.run() : statement.run(params);
    },
    async transaction(fn) {
      nativeDb.exec("BEGIN IMMEDIATE TRANSACTION;");
      try {
        const result = await Promise.resolve(fn(wrapper));
        nativeDb.exec("COMMIT;");
        return result;
      } catch (error) {
        try {
          nativeDb.exec("ROLLBACK;");
        } catch {}
        throw error;
      }
    }
  };
  return wrapper;
}
async function enableForeignKeys(db) {
  await Promise.resolve(db.run("PRAGMA foreign_keys = ON;"));
}

// electron/main/db/database.ts
var CREATE_GAMES_TABLE_SQL = `
CREATE TABLE IF NOT EXISTS games (
  id TEXT PRIMARY KEY,
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
var CREATE_PLAYTIME_DAILY_SQL = `
CREATE TABLE IF NOT EXISTS playtime_daily (
  game_id TEXT NOT NULL,
  date TEXT NOT NULL,
  seconds INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
  UNIQUE(game_id, date)
)`;
var CREATE_BACKUPS_SQL = `
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
var CREATE_SETTINGS_SQL = `
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
)`;
var CREATE_SCAN_DIRECTORIES_SQL = `
CREATE TABLE IF NOT EXISTS scan_directories (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL UNIQUE,
  last_scanned TEXT,
  auto_scan INTEGER DEFAULT 0
)`;
var CREATE_NOTIFICATIONS_SQL = `
CREATE TABLE IF NOT EXISTS notifications (
  id TEXT PRIMARY KEY,
  level TEXT NOT NULL,
  title TEXT NOT NULL,
  message TEXT NOT NULL,
  source TEXT,
  created_at TEXT NOT NULL,
  read_at TEXT
)`;
var CREATE_ACHIEVEMENTS_SQL = `
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
var CREATE_CATALOGUE_ITEMS_SQL = `
CREATE TABLE IF NOT EXISTS catalogue_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  rawg_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  payload TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'rawg',
  updated_at TEXT NOT NULL,
  UNIQUE(rawg_id, source)
)`;
var GAME_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_games_name ON games(name)",
  "CREATE INDEX IF NOT EXISTS idx_games_favorite_name ON games(is_favorite, name)"
];
var PLAYTIME_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_playtime_daily_date ON playtime_daily(date)",
  "CREATE INDEX IF NOT EXISTS idx_playtime_daily_game ON playtime_daily(game_id)"
];
var BACKUP_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_backups_game_created ON backups(game_id, created_at DESC)"
];
var NOTIFICATION_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at DESC)"
];
var ACHIEVEMENT_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_achievements_trigger ON achievements(event_trigger)",
  "CREATE INDEX IF NOT EXISTS idx_achievements_unlocked ON achievements(unlocked, unlocked_at DESC)"
];
var CATALOGUE_INDEXES = [
  "CREATE INDEX IF NOT EXISTS idx_catalogue_items_rawg_id ON catalogue_items(rawg_id)"
];
var DEFAULT_SETTINGS = [
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
  ["rawg_api_key", ""]
];
async function getTableColumns(db, tableName) {
  const rows = await queryAll(db, `PRAGMA table_info(${tableName})`);
  return new Set(rows.map((row) => row.name));
}
async function addColumnIfMissing(db, columns, name, sql) {
  if (!columns.has(name)) {
    await execute(db, sql);
    columns.add(name);
  }
}
async function ensureIndexes(db, statements) {
  for (const sql of statements) {
    await execute(db, sql);
  }
}
async function ensureDefaultSettings(db) {
  for (const [key, value] of DEFAULT_SETTINGS) {
    await execute(db, "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)", [key, value]);
  }
}
async function ensureGameColumns(db) {
  const columns = await getTableColumns(db, "games");
  await addColumnIfMissing(db, columns, "user_rating", "ALTER TABLE games ADD COLUMN user_rating INTEGER");
  await addColumnIfMissing(db, columns, "user_note", "ALTER TABLE games ADD COLUMN user_note TEXT");
  await addColumnIfMissing(db, columns, "save_path", "ALTER TABLE games ADD COLUMN save_path TEXT");
  await addColumnIfMissing(db, columns, "save_path_checked", "ALTER TABLE games ADD COLUMN save_path_checked INTEGER DEFAULT 0");
  await addColumnIfMissing(db, columns, "icon_image", "ALTER TABLE games ADD COLUMN icon_image TEXT");
  await addColumnIfMissing(db, columns, "play_status", "ALTER TABLE games ADD COLUMN play_status TEXT NOT NULL DEFAULT 'not_started'");
}
async function initAppDatabase(db) {
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
  await ensureDefaultSettings(db);
}
async function openGameDatabase(db, options = {}) {
  if (options.initialize !== false) {
    await initAppDatabase(db);
  }
  return db;
}
// electron/main/helpers/achievements.ts
var DEFAULT_ACHIEVEMENTS = [
  {
    id: "first-launch",
    title: "First Launch",
    description: "Launch any game once.",
    event_trigger: "game_launch",
    target: 1
  },
  {
    id: "backup-guardian",
    title: "Backup Guardian",
    description: "Complete a backup or restore flow.",
    event_trigger: "backup_restore",
    target: 1
  },
  {
    id: "download-scout",
    title: "Download Scout",
    description: "Register a completed download event.",
    event_trigger: "download_complete",
    target: 1
  },
  {
    id: "scan-master",
    title: "Scan Master",
    description: "Finish a library scan.",
    event_trigger: "scan_complete",
    target: 1
  }
];
function mapAchievementRow(row) {
  return {
    id: row.id,
    title: row.title,
    description: row.description,
    event_trigger: row.event_trigger,
    progress: row.progress,
    target: row.target,
    unlocked: row.unlocked === 1,
    unlocked_at: row.unlocked_at,
    created_at: row.created_at
  };
}

// electron/main/services/achievements.ts
async function seedDefaults(db) {
  const createdAt = new Date().toISOString();
  await runInTransaction(db, async (tx) => {
    for (const seed of DEFAULT_ACHIEVEMENTS) {
      await execute(tx, `INSERT OR IGNORE INTO achievements
          (id, title, description, event_trigger, progress, target, unlocked, unlocked_at, created_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, 0, NULL, ?6)`, [seed.id, seed.title, seed.description, seed.event_trigger, seed.target, createdAt]);
    }
  });
}
async function fetchAchievements(db, unlockedOnly) {
  const rows = unlockedOnly ? await queryAll(db, `SELECT id, title, description, event_trigger, progress, target, unlocked, unlocked_at, created_at
         FROM achievements
         WHERE unlocked = 1
         ORDER BY COALESCE(unlocked_at, created_at) DESC, title ASC`) : await queryAll(db, `SELECT id, title, description, event_trigger, progress, target, unlocked, unlocked_at, created_at
         FROM achievements
         ORDER BY unlocked DESC, COALESCE(unlocked_at, created_at) DESC, title ASC`);
  return rows.map(mapAchievementRow);
}
function createAchievementsService(deps) {
  return {
    async getAllAchievements(unlockedOnly) {
      await seedDefaults(deps.db);
      return await fetchAchievements(deps.db, unlockedOnly);
    },
    async seedDefaultAchievements() {
      await seedDefaults(deps.db);
      return await fetchAchievements(deps.db, false);
    },
    async recordAchievementEvent(eventTrigger, eventContext) {
      const trigger = eventTrigger.trim();
      await seedDefaults(deps.db);
      if (trigger.length === 0) {
        return await fetchAchievements(deps.db, false);
      }
      await runInTransaction(deps.db, async (tx) => {
        const unlockedAt = new Date().toISOString();
        await execute(tx, `UPDATE achievements
           SET progress = CASE WHEN progress < target THEN progress + 1 ELSE progress END,
               unlocked = CASE WHEN progress + 1 >= target THEN 1 ELSE unlocked END,
               unlocked_at = CASE
                 WHEN unlocked = 0 AND progress + 1 >= target THEN ?1
                 ELSE unlocked_at
               END
           WHERE event_trigger = ?2`, [unlockedAt, trigger]);
      });
      return await fetchAchievements(deps.db, false);
    }
  };
}

// electron/main/helpers/catalogue.ts
var FNV64_OFFSET_BASIS = 0xcbf29ce484222325n;
var FNV64_PRIME = 0x100000001b3n;
var SAFE_ID_MASK = 0x1fffffffffffffn;
function normalizeCatalogueSource(source) {
  const normalized = source?.trim();
  return normalized && normalized.length > 0 ? normalized : "rawg";
}
function validateCataloguePayload(payload) {
  if (typeof payload !== "string") {
    throw new Error("Payload must be valid JSON");
  }
  try {
    JSON.parse(payload);
  } catch {
    throw new Error("Payload must be valid JSON");
  }
}
function stableCatalogueRawgId(value) {
  const bytes = new TextEncoder().encode(value);
  let hash = FNV64_OFFSET_BASIS;
  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = hash * FNV64_PRIME & 0xffffffffffffffffn;
  }
  const safe = Number(hash & SAFE_ID_MASK);
  return safe > 0 ? safe : 1;
}
function mapCatalogueRow(row) {
  return {
    id: row.id,
    rawg_id: row.rawg_id,
    name: row.name,
    payload: row.payload,
    source: row.source,
    updated_at: row.updated_at
  };
}
function buildLibraryPayload(item) {
  return JSON.stringify({
    game_id: item.id,
    exe_name: item.exe_name
  });
}
function createCatalogueSyncResult(count) {
  return { synced: count };
}

// electron/main/services/catalogue.ts
async function listCatalogueRows(db, source) {
  if (source) {
    const rows2 = await queryAll(db, `SELECT id, rawg_id, name, payload, source, updated_at
       FROM catalogue_items
       WHERE source = ?1
       ORDER BY name ASC`, [source]);
    return rows2.map(mapCatalogueRow);
  }
  const rows = await queryAll(db, `SELECT id, rawg_id, name, payload, source, updated_at
     FROM catalogue_items
     ORDER BY name ASC`);
  return rows.map(mapCatalogueRow);
}
function createCatalogueService(deps) {
  return {
    async getCatalogueItems(source) {
      return await listCatalogueRows(deps.db, source);
    },
    async searchCatalogue(query, source) {
      const trimmed = query.trim();
      if (trimmed.length === 0) {
        return await listCatalogueRows(deps.db, source);
      }
      const pattern = `%${trimmed}%`;
      if (source) {
        const rows2 = await queryAll(deps.db, `SELECT id, rawg_id, name, payload, source, updated_at
           FROM catalogue_items
           WHERE source = ?1 AND (name LIKE ?2 OR payload LIKE ?2)
           ORDER BY name ASC`, [source, pattern]);
        return rows2.map(mapCatalogueRow);
      }
      const rows = await queryAll(deps.db, `SELECT id, rawg_id, name, payload, source, updated_at
         FROM catalogue_items
         WHERE name LIKE ?1 OR payload LIKE ?1
         ORDER BY name ASC`, [pattern]);
      return rows.map(mapCatalogueRow);
    },
    async upsertCatalogueItem(input) {
      const name = input.name.trim();
      if (name.length === 0) {
        throw new Error("Name cannot be empty");
      }
      validateCataloguePayload(input.payload);
      const source = normalizeCatalogueSource(input.source);
      if (typeof input.id === "number") {
        const updatedAt2 = new Date().toISOString();
        const changes = await execute(deps.db, `UPDATE catalogue_items
           SET rawg_id = ?1, name = ?2, payload = ?3, source = ?4, updated_at = ?5
           WHERE id = ?6`, [input.rawgId, name, input.payload, source, updatedAt2, input.id]);
        if (changes === 0) {
          throw new Error("Catalogue item not found");
        }
        const row2 = await queryOne(deps.db, `SELECT id, rawg_id, name, payload, source, updated_at
           FROM catalogue_items
           WHERE id = ?1`, [input.id]);
        if (!row2) {
          throw new Error("Catalogue item not found");
        }
        return mapCatalogueRow(row2);
      }
      const updatedAt = new Date().toISOString();
      await execute(deps.db, `INSERT INTO catalogue_items (rawg_id, name, payload, source, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(rawg_id, source) DO UPDATE
         SET name = excluded.name,
             payload = excluded.payload,
             updated_at = excluded.updated_at`, [input.rawgId, name, input.payload, source, updatedAt]);
      const row = await queryOne(deps.db, `SELECT id, rawg_id, name, payload, source, updated_at
         FROM catalogue_items
         WHERE rawg_id = ?1 AND source = ?2
         LIMIT 1`, [input.rawgId, source]);
      if (!row) {
        throw new Error("Failed to load upserted catalogue item");
      }
      return mapCatalogueRow(row);
    },
    async deleteCatalogueItem(id) {
      return await execute(deps.db, "DELETE FROM catalogue_items WHERE id = ?1", [id]) > 0;
    },
    async syncLibraryToCatalogue() {
      const libraryItems = await deps.library.listGames();
      const updatedAt = new Date().toISOString();
      const source = "library";
      await runInTransaction(deps.db, async (tx) => {
        for (const item of libraryItems) {
          const payload = buildLibraryPayload(item);
          const rawgId = stableCatalogueRawgId(item.id);
          await execute(tx, `INSERT INTO catalogue_items (rawg_id, name, payload, source, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(rawg_id, source) DO UPDATE
             SET name = excluded.name,
                 payload = excluded.payload,
                 updated_at = excluded.updated_at`, [rawgId, item.name, payload, source, updatedAt]);
        }
      });
      return createCatalogueSyncResult(libraryItems.length);
    }
  };
}

// electron/main/services/games.ts
import fs2 from "node:fs";
import path2 from "node:path";
import { randomUUID } from "node:crypto";

// electron/main/services/games/process.ts
import { execFile as execFileCb, spawn } from "node:child_process";
import { promisify } from "node:util";
import fs from "node:fs";
import path from "node:path";
var execFile = promisify(execFileCb);
var isWindows = process.platform === "win32";
var isLinux = process.platform === "linux";
function escapePowerShellSingleQuoted(value) {
  return value.replace(/'/g, "''");
}
function normalizeForComparison(input) {
  const resolved = fs.existsSync(input) ? fs.realpathSync.native(input) : path.resolve(input);
  const normalized = path.normalize(resolved);
  return isWindows ? normalized.toLowerCase() : normalized;
}
function firstCommandToken(command) {
  const trimmed = command.trim();
  if (!trimmed) {
    return "";
  }
  if (trimmed.startsWith('"')) {
    const end = trimmed.indexOf('"', 1);
    return end > 1 ? trimmed.slice(1, end) : trimmed.slice(1);
  }
  const spaceIndex = trimmed.indexOf(" ");
  return spaceIndex === -1 ? trimmed : trimmed.slice(0, spaceIndex);
}
async function listWindowsProcesses() {
  const script = `
$ErrorActionPreference = 'Stop'
Get-CimInstance Win32_Process |
  Select-Object ProcessId, Name, ExecutablePath |
  ConvertTo-Json -Compress
`.trim();
  const { stdout } = await execFile("powershell.exe", [
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    script
  ]);
  if (!stdout.trim()) {
    return [];
  }
  const payload = JSON.parse(stdout);
  const items = Array.isArray(payload) ? payload : [payload];
  return items.map((item) => ({
    pid: Number(item.ProcessId),
    name: item.Name ?? "",
    path: item.ExecutablePath ?? ""
  }));
}
async function listUnixProcesses() {
  if (isLinux) {
    const entries = await fs.promises.readdir("/proc", { withFileTypes: true });
    const result = [];
    for (const entry of entries) {
      if (!entry.isDirectory() || !/^\d+$/.test(entry.name)) {
        continue;
      }
      const pid = Number(entry.name);
      const procDir = path.join("/proc", entry.name);
      const exePath = path.join(procDir, "exe");
      const commPath = path.join(procDir, "comm");
      try {
        const resolved = await fs.promises.readlink(exePath);
        const name = (await fs.promises.readFile(commPath, "utf8")).trim();
        result.push({
          pid,
          name,
          path: resolved
        });
      } catch {
        const cmdlinePath = path.join(procDir, "cmdline");
        try {
          const raw = await fs.promises.readFile(cmdlinePath, "utf8");
          const command = raw.replace(/\0/g, " ").trim();
          const token = firstCommandToken(command);
          result.push({
            pid,
            name: path.basename(token || command),
            path: token
          });
        } catch {}
      }
    }
    return result;
  }
  const { stdout } = await execFile("ps", ["-axo", "pid=,command="]);
  return stdout.split(/\r?\n/).map((line) => line.trim()).filter(Boolean).map((line) => {
    const match = line.match(/^(\d+)\s+(.*)$/);
    if (!match) {
      return null;
    }
    const pid = Number(match[1]);
    const command = match[2];
    const token = firstCommandToken(command);
    return {
      pid,
      name: path.basename(token || command),
      path: token
    };
  }).filter((value) => value !== null);
}
async function listRunningProcesses() {
  if (isWindows) {
    return listWindowsProcesses();
  }
  return listUnixProcesses();
}
async function countRunningInstances(exePath) {
  const target = normalizeForComparison(exePath);
  const processes = await listRunningProcesses();
  return processes.filter((process2) => {
    if (!process2.path) {
      return false;
    }
    return normalizeForComparison(process2.path) === target;
  }).length;
}
async function killMatchingProcesses(exePath) {
  const target = normalizeForComparison(exePath);
  const processes = await listRunningProcesses();
  let killed = 0;
  for (const processInfo of processes) {
    if (!processInfo.path) {
      continue;
    }
    if (normalizeForComparison(processInfo.path) !== target) {
      continue;
    }
    try {
      process.kill(processInfo.pid);
      killed += 1;
    } catch {}
  }
  return killed;
}
async function resolveShortcutTarget(pathname) {
  if (!isWindows || !pathname.toLowerCase().endsWith(".lnk")) {
    return pathname;
  }
  const script = `
$ErrorActionPreference = 'Stop'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut('${escapePowerShellSingleQuoted(pathname)}')
[Console]::Out.Write($shortcut.TargetPath)
`.trim();
  try {
    const { stdout } = await execFile("powershell.exe", [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      script
    ]);
    const resolved = stdout.trim();
    return resolved || pathname;
  } catch {
    return pathname;
  }
}
async function spawnGameProcess(exePath) {
  const cwd = path.dirname(exePath);
  await new Promise((resolve, reject) => {
    const child = spawn(exePath, {
      cwd,
      detached: true,
      stdio: "ignore",
      windowsHide: true
    });
    child.once("error", (error) => reject(error));
    child.once("spawn", () => {
      child.unref();
      resolve();
    });
  });
}

// electron/main/services/games.ts
var GAME_SELECT = `SELECT
  id, name, exe_path, exe_name, rawg_id, description, released,
  background_image, metacritic, rating, genres, platforms, developers, publishers,
  cover_image, icon_image, is_favorite, play_count, total_playtime, last_played, date_added,
  backup_enabled, last_backup, backup_count, save_path, user_rating, user_note, play_status
FROM games`;
var GAME_PATH_TOKEN = "{PATHTOGAME}";
function readString(row, key) {
  const value = row[key];
  return typeof value === "string" ? value : "";
}
function readStringOrNull(row, key) {
  const value = row[key];
  return typeof value === "string" ? value : null;
}
function readNumberOrNull(row, key) {
  const value = row[key];
  if (value === null || value === undefined) {
    return null;
  }
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}
function readNumber(row, key) {
  const value = row[key];
  const number = Number(value ?? 0);
  return Number.isFinite(number) ? number : 0;
}
function readBoolean(row, key) {
  return Number(row[key] ?? 0) === 1;
}
function mapGameRow(row) {
  return {
    id: readString(row, "id"),
    name: readString(row, "name"),
    exe_path: readString(row, "exe_path"),
    exe_name: readString(row, "exe_name"),
    play_status: readStringOrNull(row, "play_status") ?? "not_started",
    rawg_id: readNumberOrNull(row, "rawg_id"),
    description: readStringOrNull(row, "description"),
    released: readStringOrNull(row, "released"),
    background_image: readStringOrNull(row, "background_image"),
    metacritic: readNumberOrNull(row, "metacritic"),
    rating: readNumberOrNull(row, "rating"),
    genres: readStringOrNull(row, "genres"),
    platforms: readStringOrNull(row, "platforms"),
    developers: readStringOrNull(row, "developers"),
    publishers: readStringOrNull(row, "publishers"),
    cover_image: readStringOrNull(row, "cover_image"),
    icon_image: readStringOrNull(row, "icon_image"),
    is_favorite: readBoolean(row, "is_favorite"),
    play_count: readNumber(row, "play_count"),
    total_playtime: readNumber(row, "total_playtime"),
    last_played: readStringOrNull(row, "last_played"),
    date_added: readString(row, "date_added"),
    backup_enabled: readBoolean(row, "backup_enabled"),
    last_backup: readStringOrNull(row, "last_backup"),
    backup_count: readNumber(row, "backup_count"),
    save_path: readStringOrNull(row, "save_path"),
    user_rating: readNumberOrNull(row, "user_rating"),
    user_note: readStringOrNull(row, "user_note")
  };
}
async function getRowById(db, id) {
  const row = await queryOne(db, `${GAME_SELECT} WHERE id = ?1`, [id]);
  return row ? mapGameRow(row) : null;
}
async function fetchExePath(db, id) {
  const row = await queryOne(db, "SELECT exe_path FROM games WHERE id = ?1", [
    id
  ]);
  if (!row) {
    throw new Error("Game not found");
  }
  return row.exe_path;
}
async function normalizeSavePathIfPossible(db, gameId, savePath) {
  if (savePath.includes(GAME_PATH_TOKEN)) {
    return savePath;
  }
  const absolutePath = path2.isAbsolute(savePath) ? savePath : "";
  if (!absolutePath || !fs2.existsSync(absolutePath)) {
    return savePath;
  }
  const row = await queryOne(db, "SELECT exe_path FROM games WHERE id = ?1", [
    gameId
  ]);
  if (!row) {
    return savePath;
  }
  const gameDir = path2.dirname(row.exe_path);
  let canonicalGameDir;
  let canonicalSavePath;
  try {
    canonicalGameDir = fs2.realpathSync.native(gameDir);
    canonicalSavePath = fs2.realpathSync.native(absolutePath);
  } catch {
    return savePath;
  }
  const relative = path2.relative(canonicalGameDir, canonicalSavePath);
  if (!relative || relative === "") {
    return GAME_PATH_TOKEN;
  }
  if (relative.startsWith("..")) {
    return savePath;
  }
  return path2.join(GAME_PATH_TOKEN, relative);
}
async function prepareGameInsert(db, game, id, dateAdded) {
  await execute(db, "INSERT INTO games (id, name, exe_path, exe_name, date_added) VALUES (?1, ?2, ?3, ?4, ?5)", [id, game.name, game.exe_path, game.exe_name, dateAdded]);
}
async function buildUpdateClause(update, db) {
  const updates = [];
  const values = [];
  const push = (column, value) => {
    updates.push(`${column} = ?`);
    values.push(value);
  };
  if (update.name !== undefined) {
    if (update.name === null || update.name.trim() === "") {
      throw new Error("name cannot be empty");
    }
    push("name", update.name);
  }
  if (update.exe_path !== undefined) {
    if (update.exe_path === null || update.exe_path.trim() === "") {
      throw new Error("exe_path cannot be empty");
    }
    const normalized = update.exe_path.trim();
    const exeName = path2.basename(normalized);
    if (!exeName) {
      throw new Error("Invalid exe path");
    }
    push("exe_path", normalized);
    push("exe_name", exeName);
  }
  if (update.description !== undefined) {
    push("description", update.description);
  }
  if (update.cover_image !== undefined) {
    push("cover_image", update.cover_image);
  }
  if (update.icon_image !== undefined) {
    push("icon_image", update.icon_image);
  }
  if (update.is_favorite !== undefined) {
    push("is_favorite", update.is_favorite ? 1 : 0);
  }
  if (update.backup_enabled !== undefined) {
    push("backup_enabled", update.backup_enabled ? 1 : 0);
  }
  if (update.save_path !== undefined) {
    const normalized = update.save_path === null || update.save_path.trim() === "" ? null : await normalizeSavePathIfPossible(db, update.id, update.save_path);
    push("save_path", normalized);
    push("save_path_checked", normalized !== null ? 1 : 0);
  }
  if (update.rawg_id !== undefined) {
    push("rawg_id", update.rawg_id);
  }
  if (update.released !== undefined) {
    push("released", update.released);
  }
  if (update.background_image !== undefined) {
    push("background_image", update.background_image);
  }
  if (update.metacritic !== undefined) {
    push("metacritic", update.metacritic);
  }
  if (update.rating !== undefined) {
    push("rating", update.rating);
  }
  if (update.genres !== undefined) {
    push("genres", update.genres);
  }
  if (update.platforms !== undefined) {
    push("platforms", update.platforms);
  }
  if (update.developers !== undefined) {
    push("developers", update.developers);
  }
  if (update.publishers !== undefined) {
    push("publishers", update.publishers);
  }
  if (update.user_rating !== undefined) {
    push("user_rating", update.user_rating);
  }
  if (update.user_note !== undefined) {
    push("user_note", update.user_note);
  }
  if (update.play_status !== undefined) {
    push("play_status", update.play_status);
  }
  if (updates.length === 0) {
    return { sql: "", values: [] };
  }
  values.push(update.id);
  return {
    sql: `UPDATE games SET ${updates.join(", ")} WHERE id = ?`,
    values
  };
}
function isUniqueConstraintError(error) {
  return String(error).includes("UNIQUE constraint failed");
}
function createGamesService(deps) {
  const usageReadModel = deps.usageReadModel;
  const now = deps.now ?? (() => new Date);
  const log = deps.log ?? console;
  const exists = deps.fileExists ?? fs2.existsSync;
  const resolveShortcut = deps.resolveShortcutTarget ?? resolveShortcutTarget;
  const countInstances = deps.countRunningInstances ?? countRunningInstances;
  const killProcesses = deps.killMatchingProcesses ?? killMatchingProcesses;
  const spawnProcess = deps.spawnGameProcess ?? spawnGameProcess;
  const recordGameLaunch = async (id) => {
    const launchedAt = now().toISOString();
    await execute(deps.db, "UPDATE games SET play_count = play_count + 1, last_played = ?1 WHERE id = ?2", [launchedAt, id]);
    const fetched = await getRowById(deps.db, id);
    if (!fetched) {
      throw new Error("Game not found");
    }
    return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
  };
  return {
    async getGame(id) {
      const game = await getRowById(deps.db, id);
      if (!game) {
        return null;
      }
      return usageReadModel ? await usageReadModel.hydrateGame(game) : game;
    },
    async addGame(game) {
      const id = randomUUID();
      const dateAdded = now().toISOString();
      await prepareGameInsert(deps.db, game, id, dateAdded);
      const fetched = await getRowById(deps.db, id);
      if (!fetched) {
        throw new Error("Failed to fetch inserted game");
      }
      return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
    },
    async addGamesBatch(games) {
      if (games.length === 0) {
        return [];
      }
      const inserted = [];
      await runInTransaction(deps.db, async (tx) => {
        for (const game of games) {
          const id = randomUUID();
          const dateAdded = now().toISOString();
          try {
            await prepareGameInsert(tx, game, id, dateAdded);
            inserted.push({ id, name: game.name });
          } catch (error) {
            if (!isUniqueConstraintError(error)) {
              log.error?.(`Error adding game ${game.name}:`, error);
            }
          }
        }
      });
      const result = [];
      for (const item of inserted) {
        const fetched = await getRowById(deps.db, item.id);
        if (fetched) {
          result.push(usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched);
        } else {
          log.error?.(`Error fetching new game ${item.id}:`, item.name);
        }
      }
      return result;
    },
    async getAllGames() {
      const rows = await queryAll(deps.db, `${GAME_SELECT} ORDER BY name ASC`);
      const games = rows.map(mapGameRow);
      return usageReadModel ? await usageReadModel.hydrateGames(games) : games;
    },
    async getFavorites() {
      const rows = await queryAll(deps.db, `${GAME_SELECT} WHERE is_favorite = 1 ORDER BY name ASC`);
      const games = rows.map(mapGameRow);
      return usageReadModel ? await usageReadModel.hydrateGames(games) : games;
    },
    async updateGame(update) {
      const { sql, values } = await buildUpdateClause(update, deps.db);
      if (sql) {
        await execute(deps.db, sql, values);
      }
      const fetched = await getRowById(deps.db, update.id);
      if (!fetched) {
        throw new Error("Game not found");
      }
      return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
    },
    async toggleFavorite(id) {
      await execute(deps.db, "UPDATE games SET is_favorite = CASE WHEN is_favorite = 1 THEN 0 ELSE 1 END WHERE id = ?1", [id]);
      const fetched = await getRowById(deps.db, id);
      if (!fetched) {
        throw new Error("Game not found");
      }
      return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
    },
    async deleteGame(id) {
      await execute(deps.db, "DELETE FROM games WHERE id = ?1", [id]);
    },
    async recordGameLaunch(id) {
      return await recordGameLaunch(id);
    },
    async searchGames(query) {
      const pattern = `%${query}%`;
      const rows = await queryAll(deps.db, `${GAME_SELECT} WHERE name LIKE ?1 OR exe_name LIKE ?1 ORDER BY name ASC`, [pattern]);
      const games = rows.map(mapGameRow);
      return usageReadModel ? await usageReadModel.hydrateGames(games) : games;
    },
    async gameExistsByPath(exePath) {
      const row = await queryOne(deps.db, "SELECT COUNT(*) AS count FROM games WHERE exe_path = ?1", [exePath]);
      return Number(row?.count ?? 0) > 0;
    },
    async resolveShortcutTarget(pathname) {
      return await resolveShortcut(pathname);
    },
    async isGameInstalled(id) {
      const exePath = await fetchExePath(deps.db, id);
      return exists(exePath);
    },
    async getRunningInstances(id) {
      const exePath = await fetchExePath(deps.db, id);
      return await countInstances(exePath);
    },
    async killGameProcesses(id) {
      const exePath = await fetchExePath(deps.db, id);
      return await killProcesses(exePath);
    },
    async launchGame(id) {
      const exePath = await fetchExePath(deps.db, id);
      await spawnProcess(exePath);
      await recordGameLaunch(id);
    }
  };
}

// electron/main/helpers/rawg.ts
var RAWG_API_BASE = "https://api.rawg.io/api";
var DEFAULT_TIMEOUT_MS = 15000;
var DEFAULT_USER_AGENT = "Arrancador/0.1.0";
function joinNames(items) {
  if (!items || items.length === 0) {
    return null;
  }
  return items.map((item) => item.name).join(", ");
}
function buildApiUrl(path3, apiKey) {
  const normalizedPath = path3.startsWith("/") ? path3.slice(1) : path3;
  const url = new URL(normalizedPath, `${RAWG_API_BASE}/`);
  if (apiKey && apiKey.trim().length > 0) {
    url.searchParams.set("key", apiKey.trim());
  }
  return url.toString();
}
async function requestJson(fetchImpl, apiKey, path3, timeoutMs, userAgent) {
  const url = buildApiUrl(path3, apiKey);
  const controller = new AbortController;
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetchImpl(url, {
      signal: controller.signal,
      headers: {
        "User-Agent": userAgent
      }
    });
    if (!response.ok) {
      throw new Error(`API error: ${response.status}`);
    }
    return await response.json();
  } catch (error) {
    if (error instanceof Error && error.name === "AbortError") {
      throw new Error("RAWG request timed out");
    }
    if (error instanceof TypeError) {
      throw new Error(`Network error: ${error.message}`);
    }
    if (error instanceof Error) {
      throw new Error(error.message.startsWith("API error:") ? error.message : `Parse error: ${error.message}`);
    }
    throw new Error("Unexpected RAWG request failure");
  } finally {
    clearTimeout(timeout);
  }
}
function createRawgClient(options) {
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  if (!fetchImpl) {
    throw new Error("A fetch implementation is required for RAWG requests");
  }
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const userAgent = options.userAgent ?? DEFAULT_USER_AGENT;
  return {
    async searchRawg(query) {
      const apiKey = await options.getApiKey();
      const params = new URLSearchParams({
        search: query,
        page_size: "10"
      });
      const path3 = `games?${params.toString()}`;
      const result = await requestJson(fetchImpl, apiKey, path3, timeoutMs, userAgent);
      return result.results;
    },
    async getRawgGameDetails(rawgId) {
      const apiKey = await options.getApiKey();
      return await requestJson(fetchImpl, apiKey, `games/${rawgId}`, timeoutMs, userAgent);
    }
  };
}
function buildRawgMetadataSummary(details) {
  return {
    name: details.name ?? null,
    description: details.description_raw ?? details.description ?? null,
    released: details.released ?? null,
    background_image: details.background_image ?? null,
    metacritic: details.metacritic ?? null,
    rating: details.rating ?? null,
    genres: joinNames(details.genres),
    platforms: joinNames(details.platforms?.map((item) => item.platform) ?? null),
    developers: joinNames(details.developers),
    publishers: joinNames(details.publishers)
  };
}

// electron/main/services/metadata.ts
function createMetadataUpdate(gameId, rawgId, rename, details) {
  const summary = buildRawgMetadataSummary(details);
  return {
    gameId,
    rawgId,
    name: rename ? summary.name : null,
    description: summary.description,
    released: summary.released,
    background_image: summary.background_image,
    metacritic: summary.metacritic,
    rating: summary.rating,
    genres: summary.genres,
    platforms: summary.platforms,
    developers: summary.developers,
    publishers: summary.publishers
  };
}
function createMetadataService(deps) {
  const rawg = deps.rawg ?? createRawgClient({
    getApiKey: () => deps.settings.getRawgApiKey(),
    fetchImpl: deps.fetchImpl,
    timeoutMs: deps.timeoutMs,
    userAgent: deps.userAgent
  });
  return {
    async searchRawg(query) {
      return await rawg.searchRawg(query);
    },
    async getRawgGameDetails(rawgId) {
      return await rawg.getRawgGameDetails(rawgId);
    },
    async applyRawgMetadata(gameId, rawgId, rename) {
      const details = await rawg.getRawgGameDetails(rawgId);
      const update = createMetadataUpdate(gameId, rawgId, rename, details);
      return await deps.games.applyRawgMetadata(update);
    },
    async setRawgApiKey(key) {
      await deps.settings.setRawgApiKey(key);
    },
    async getRawgApiKey() {
      return await deps.settings.getRawgApiKey() ?? "";
    }
  };
}

// electron/main/helpers/notifications.ts
import { randomUUID as randomUUID2 } from "node:crypto";
function createNotificationId() {
  return randomUUID2();
}
function mapNotificationRow(row) {
  return {
    id: row.id,
    level: row.level,
    title: row.title,
    message: row.message,
    source: row.source,
    created_at: row.created_at,
    read_at: row.read_at
  };
}

// electron/main/services/notifications.ts
async function insertNotification(db, level, title, message, source) {
  const id = createNotificationId();
  const createdAt = new Date().toISOString();
  const normalizedSource = source?.trim() ? source.trim() : null;
  await execute(db, `INSERT INTO notifications (id, level, title, message, source, created_at, read_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)`, [id, level, title, message, normalizedSource, createdAt]);
  return {
    id,
    level,
    title,
    message,
    source: normalizedSource,
    created_at: createdAt,
    read_at: null
  };
}
function createNotificationsService(deps) {
  return {
    async listNotifications(unreadOnly = false) {
      const rows = unreadOnly ? await queryAll(deps.db, `SELECT id, level, title, message, source, created_at, read_at
             FROM notifications
             WHERE read_at IS NULL
             ORDER BY created_at DESC
             LIMIT 200`) : await queryAll(deps.db, `SELECT id, level, title, message, source, created_at, read_at
             FROM notifications
             ORDER BY created_at DESC
             LIMIT 200`);
      return rows.map(mapNotificationRow);
    },
    async createNotification(level, title, message, source) {
      return await insertNotification(deps.db, level, title, message, source);
    },
    async markNotificationRead(id) {
      const updated = await execute(deps.db, "UPDATE notifications SET read_at = ?1 WHERE id = ?2 AND read_at IS NULL", [new Date().toISOString(), id]);
      return updated > 0;
    },
    async markAllNotificationsRead() {
      return await execute(deps.db, "UPDATE notifications SET read_at = ?1 WHERE read_at IS NULL", [new Date().toISOString()]);
    },
    async clearNotifications() {
      const rows = await queryAll(deps.db, "SELECT id FROM notifications");
      await execute(deps.db, "DELETE FROM notifications");
      return rows.length;
    },
    async notifySuccess(title, message) {
      return await insertNotification(deps.db, "success", title, message, null);
    },
    async notifyInfo(title, message) {
      return await insertNotification(deps.db, "info", title, message, null);
    },
    async notifyWarning(title, message) {
      return await insertNotification(deps.db, "warning", title, message, null);
    },
    async notifyError(title, message) {
      return await insertNotification(deps.db, "error", title, message, null);
    }
  };
}

// electron/main/services/ark-usage.ts
function normalizeExePath(exePath) {
  return exePath.trim().replaceAll("/", "\\").toLowerCase();
}
function buildParameterizedList(values, startIndex = 1) {
  return values.map((_, index) => `?${index + startIndex}`).join(", ");
}
function createLegacyPlaytimeStatsRepository(db) {
  return {
    async getDailyTotals(rangeStart, rangeEnd) {
      return await queryAll(db, `SELECT date, SUM(seconds) AS seconds
         FROM playtime_daily
         WHERE date BETWEEN ?1 AND ?2
         GROUP BY date
         ORDER BY date`, [rangeStart, rangeEnd]);
    },
    async getPerGameTotals(rangeStart, rangeEnd) {
      return await queryAll(db, `SELECT games.id, games.name, SUM(playtime_daily.seconds) AS seconds
         FROM playtime_daily
         JOIN games ON games.id = playtime_daily.game_id
         WHERE playtime_daily.date BETWEEN ?1 AND ?2
         GROUP BY games.id, games.name
         HAVING seconds > 0
         ORDER BY seconds DESC`, [rangeStart, rangeEnd]);
    }
  };
}
async function loadGamePathIndex(legacyDb, ids) {
  if (ids && ids.length === 0) {
    return [];
  }
  if (ids && ids.length > 0) {
    const placeholders = buildParameterizedList(ids);
    return await queryAll(legacyDb, `SELECT id, name, LOWER(REPLACE(exe_path, '/', '\\')) AS normalized_path
       FROM games
       WHERE id IN (${placeholders})
       ORDER BY name ASC`, ids);
  }
  return await queryAll(legacyDb, `SELECT id, name, LOWER(REPLACE(exe_path, '/', '\\')) AS normalized_path
     FROM games
     ORDER BY name ASC`);
}
function tryOpenArkDb(arkDbPath) {
  try {
    return openSqliteDatabase(arkDbPath, {
      readonly: true,
      fileMustExist: true,
      timeoutMs: 2000
    });
  } catch {
    return null;
  }
}
async function queryUsageAggregatesByPath(arkDb, normalizedPaths) {
  if (normalizedPaths.length === 0) {
    return new Map;
  }
  const placeholders = buildParameterizedList(normalizedPaths);
  const rows = await queryAll(arkDb, `SELECT tracked_apps.normalized_exe_path AS normalized_path,
            CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS total_seconds,
            MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_played
     FROM usage_sessions
     JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
     WHERE tracked_apps.normalized_exe_path IN (${placeholders})
     GROUP BY tracked_apps.normalized_exe_path`, normalizedPaths);
  return new Map(rows.map((row) => [
    row.normalized_path,
    {
      totalSeconds: Number(row.total_seconds ?? 0),
      lastPlayed: row.last_played ?? null
    }
  ]));
}
async function queryDailyTotals(arkDb, normalizedPaths, rangeStart, rangeEnd) {
  if (normalizedPaths.length === 0) {
    return [];
  }
  const placeholders = buildParameterizedList(normalizedPaths);
  const dateStartIndex = normalizedPaths.length + 1;
  const dateEndIndex = normalizedPaths.length + 2;
  return await queryAll(arkDb, `SELECT SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10) AS date,
            CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS seconds
     FROM usage_sessions
     JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
     WHERE tracked_apps.normalized_exe_path IN (${placeholders})
       AND SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10)
           BETWEEN ?${dateStartIndex} AND ?${dateEndIndex}
     GROUP BY date
     ORDER BY date`, [...normalizedPaths, rangeStart, rangeEnd]);
}
async function queryPerGameTotals(arkDb, gameIndex, rangeStart, rangeEnd) {
  if (gameIndex.length === 0) {
    return [];
  }
  const normalizedPaths = gameIndex.map((row) => row.normalized_path);
  const placeholders = buildParameterizedList(normalizedPaths);
  const dateStartIndex = normalizedPaths.length + 1;
  const dateEndIndex = normalizedPaths.length + 2;
  const rows = await queryAll(arkDb, `SELECT tracked_apps.normalized_exe_path AS normalized_path,
            CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS seconds
     FROM usage_sessions
     JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
     WHERE tracked_apps.normalized_exe_path IN (${placeholders})
       AND SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10)
           BETWEEN ?${dateStartIndex} AND ?${dateEndIndex}
     GROUP BY tracked_apps.normalized_exe_path
     HAVING seconds > 0
     ORDER BY seconds DESC`, [...normalizedPaths, rangeStart, rangeEnd]);
  const byPath = new Map(rows.map((row) => [row.normalized_path, Number(row.seconds ?? 0)]));
  return gameIndex.map((game) => ({
    id: game.id,
    name: game.name,
    seconds: byPath.get(game.normalized_path) ?? 0
  })).filter((game) => game.seconds > 0).sort((left, right) => right.seconds - left.seconds);
}
function createGameUsageReadModel({
  legacyDb,
  arkDbPath
}) {
  const hydrateGames = async (games) => {
    if (games.length === 0) {
      return games;
    }
    const arkDb = tryOpenArkDb(arkDbPath);
    if (!arkDb) {
      return games;
    }
    try {
      const metricsByPath = await queryUsageAggregatesByPath(arkDb, games.map((game) => normalizeExePath(game.exe_path)));
      return games.map((game) => {
        const metrics = metricsByPath.get(normalizeExePath(game.exe_path));
        if (!metrics) {
          return game;
        }
        return {
          ...game,
          total_playtime: metrics.totalSeconds,
          last_played: metrics.lastPlayed
        };
      });
    } catch {
      return games;
    }
  };
  return {
    async hydrateGame(game) {
      const [hydrated] = await hydrateGames([game]);
      return hydrated;
    },
    hydrateGames
  };
}
function createPlaytimeStatsRepository({
  legacyDb,
  arkDbPath
}) {
  const legacy = createLegacyPlaytimeStatsRepository(legacyDb);
  return {
    async getDailyTotals(rangeStart, rangeEnd) {
      const arkDb = tryOpenArkDb(arkDbPath);
      if (!arkDb) {
        return await legacy.getDailyTotals(rangeStart, rangeEnd);
      }
      try {
        const gameIndex = await loadGamePathIndex(legacyDb);
        const rows = await queryDailyTotals(arkDb, gameIndex.map((game) => game.normalized_path), rangeStart, rangeEnd);
        return rows.length > 0 ? rows.map((row) => ({
          date: row.date,
          seconds: Number(row.seconds ?? 0)
        })) : await legacy.getDailyTotals(rangeStart, rangeEnd);
      } catch {
        return await legacy.getDailyTotals(rangeStart, rangeEnd);
      }
    },
    async getPerGameTotals(rangeStart, rangeEnd) {
      const arkDb = tryOpenArkDb(arkDbPath);
      if (!arkDb) {
        return await legacy.getPerGameTotals(rangeStart, rangeEnd);
      }
      try {
        const gameIndex = await loadGamePathIndex(legacyDb);
        const rows = await queryPerGameTotals(arkDb, gameIndex, rangeStart, rangeEnd);
        return rows.length > 0 ? rows : await legacy.getPerGameTotals(rangeStart, rangeEnd);
      } catch {
        return await legacy.getPerGameTotals(rangeStart, rangeEnd);
      }
    }
  };
}

// electron/main/services/playtime-stats.ts
function createPlaytimeStatsRepository2(db, arkDbPath) {
  return createPlaytimeStatsRepository({
    legacyDb: db,
    arkDbPath
  });
}

// electron/main/services/helpers/scan.ts
import { stat } from "node:fs/promises";
import path3 from "node:path";
function isHiddenSegment(segment) {
  return segment.startsWith(".");
}
function isExecutableFile(fileName) {
  return path3.extname(fileName).toLowerCase() === ".exe";
}
function normalizeScanRoot(root) {
  const trimmed = root.trim();
  if (!trimmed) {
    throw new Error("Empty scan directory");
  }
  return path3.resolve(trimmed);
}
function createScanAbortError() {
  const error = new Error("Scan cancelled");
  error.name = "AbortError";
  return error;
}
async function isReadableDirectory(target) {
  try {
    return (await stat(target)).isDirectory();
  } catch {
    return false;
  }
}

// electron/main/services/helpers/process.ts
import { execFile as execFile2 } from "node:child_process";
import { readlink, stat as stat2 } from "node:fs/promises";
import os from "node:os";
import path4 from "node:path";
import { promisify as promisify2 } from "node:util";
var execFileAsync = promisify2(execFile2);
function toNumber(value) {
  if (typeof value === "number") {
    return value;
  }
  if (typeof value === "string") {
    const parsed = Number(value);
    return Number.isNaN(parsed) ? 0 : parsed;
  }
  return 0;
}
function toStringValue(value) {
  return typeof value === "string" ? value : "";
}
function extractExecutableCandidate(args) {
  const trimmed = args.trim();
  if (!trimmed) {
    return "";
  }
  if (trimmed.startsWith('"')) {
    const endQuote = trimmed.indexOf('"', 1);
    if (endQuote > 1) {
      return trimmed.slice(1, endQuote);
    }
  }
  return trimmed.split(/\s+/)[0]?.replace(/^"|"$/g, "") ?? "";
}
function normalizeJsonPayload(payload) {
  if (!payload) {
    return [];
  }
  return Array.isArray(payload) ? payload : [payload];
}
async function runPowerShellJson(script) {
  const { stdout } = await execFileAsync("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script], {
    maxBuffer: 16 * 1024 * 1024,
    windowsHide: true
  });
  const text = stdout.toString("utf8").trim();
  if (!text) {
    return [];
  }
  return normalizeJsonPayload(JSON.parse(text));
}
async function listWindowsProcesses2() {
  try {
    const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$processes = Get-CimInstance Win32_Process | Select-Object ProcessId, Name, ExecutablePath
$cpu = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Select-Object IDProcess, PercentProcessorTime
[PSCustomObject]@{ processes = $processes; cpu = $cpu } | ConvertTo-Json -Depth 4 -Compress
`.trim();
    const [bundle] = await runPowerShellJson(script);
    const cpuMap = new Map;
    for (const row of normalizeJsonPayload(bundle?.cpu)) {
      const pid = toNumber(row.IDProcess);
      if (pid > 0) {
        cpuMap.set(pid, toNumber(row.PercentProcessorTime));
      }
    }
    const logicalCoreFallback = os.cpus().length || 1;
    const logicalCores = Math.max(1, os.availableParallelism?.() ?? logicalCoreFallback);
    const processes = [];
    for (const row of normalizeJsonPayload(bundle?.processes)) {
      const pid = toNumber(row.ProcessId);
      const exePath = toStringValue(row.ExecutablePath);
      if (pid <= 0 || !exePath) {
        continue;
      }
      try {
        if (!(await stat2(exePath)).isFile()) {
          continue;
        }
      } catch {
        continue;
      }
      processes.push({
        pid,
        name: toStringValue(row.Name) || path4.basename(exePath),
        path: exePath,
        cpu_usage: (cpuMap.get(pid) ?? 0) / logicalCores,
        gpu_usage: 0
      });
    }
    processes.sort((a, b) => b.cpu_usage - a.cpu_usage);
    return processes;
  } catch {
    return [];
  }
}
async function listPosixProcesses() {
  try {
    const { stdout } = await execFileAsync("ps", process.platform === "darwin" ? ["-axo", "pid=,pcpu=,comm=,args="] : ["-eo", "pid=,pcpu=,comm=,args="], {
      maxBuffer: 16 * 1024 * 1024
    });
    const lines = stdout.toString("utf8").split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
    const processes = [];
    for (const line of lines) {
      const match = /^(\d+)\s+([\d.]+)\s+(\S+)\s+(.*)$/.exec(line);
      if (!match) {
        continue;
      }
      const pid = Number(match[1]);
      const cpuUsage = Number(match[2]);
      const comm = match[3];
      const args = match[4].trim();
      if (pid <= 0) {
        continue;
      }
      let exePath = "";
      if (process.platform === "linux") {
        try {
          exePath = await readlink(`/proc/${pid}/exe`);
        } catch {
          exePath = "";
        }
      } else {
        exePath = extractExecutableCandidate(args);
      }
      if (!exePath) {
        continue;
      }
      try {
        if (!(await stat2(exePath)).isFile()) {
          continue;
        }
      } catch {
        continue;
      }
      processes.push({
        pid,
        name: comm || path4.basename(exePath),
        path: exePath,
        cpu_usage: cpuUsage,
        gpu_usage: 0
      });
    }
    processes.sort((a, b) => b.cpu_usage - a.cpu_usage);
    return processes;
  } catch {
    return [];
  }
}
async function listRunningProcesses2() {
  if (process.platform === "win32") {
    return listWindowsProcesses2();
  }
  return listPosixProcesses();
}

// electron/main/services/scan.ts
import { opendir } from "node:fs/promises";
import path5 from "node:path";
async function* scanExecutables(root, options = {}) {
  const scanRoot = normalizeScanRoot(root);
  const stack = [scanRoot];
  while (stack.length > 0) {
    if (options.signal?.aborted) {
      throw createScanAbortError();
    }
    const currentDir = stack.pop();
    if (!currentDir || !await isReadableDirectory(currentDir)) {
      continue;
    }
    const dir = await opendir(currentDir);
    try {
      for await (const entry of dir) {
        if (options.signal?.aborted) {
          throw createScanAbortError();
        }
        if (isHiddenSegment(entry.name)) {
          continue;
        }
        const fullPath = path5.join(currentDir, entry.name);
        if (entry.isDirectory()) {
          stack.push(fullPath);
          continue;
        }
        if (!entry.isFile() || !isExecutableFile(entry.name)) {
          continue;
        }
        yield {
          path: fullPath,
          file_name: entry.name
        };
      }
    } finally {
      await dir.close().catch(() => {
        return;
      });
    }
  }
}
async function scanExecutablesStream(root, options = {}) {
  let count = 0;
  for await (const entry of scanExecutables(root, options)) {
    count += 1;
    await options.onEntry?.(entry);
  }
  return count;
}
async function getRunningProcesses() {
  return listRunningProcesses2();
}
function createScanCancellation() {
  const controller = new AbortController;
  return {
    signal: controller.signal,
    cancel: () => controller.abort()
  };
}

// electron/main/services/helpers/settings.ts
var DEFAULT_SETTINGS2 = {
  theme: "system",
  ludusavi_path: "",
  backup_directory: "",
  auto_backup: true,
  backup_before_launch: false,
  backup_compression_enabled: true,
  backup_compression_level: 60,
  backup_skip_compression_once: false,
  max_backups_per_game: 5,
  rawg_api_key: "",
  start_minimized_in_tray: false
};
function clamp(value, min, max) {
  if (Number.isNaN(value)) {
    return min;
  }
  return Math.min(max, Math.max(min, value));
}
function parseBoolean(value, fallback) {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "number") {
    return value !== 0;
  }
  if (typeof value === "string") {
    return ["true", "1", "yes", "on"].includes(value.trim().toLowerCase());
  }
  return fallback;
}
function parseIntValue(value, fallback, min, max) {
  if (typeof value === "number" && Number.isFinite(value)) {
    return clamp(Math.trunc(value), min, max);
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number.parseInt(value, 10);
    if (!Number.isNaN(parsed)) {
      return clamp(parsed, min, max);
    }
  }
  return fallback;
}
function getDefaultAppSettings() {
  return { ...DEFAULT_SETTINGS2 };
}
function normalizeAppSettings(input = {}) {
  return {
    theme: typeof input.theme === "string" && input.theme.trim() ? input.theme : DEFAULT_SETTINGS2.theme,
    ludusavi_path: typeof input.ludusavi_path === "string" ? input.ludusavi_path : "",
    backup_directory: typeof input.backup_directory === "string" ? input.backup_directory : "",
    auto_backup: parseBoolean(input.auto_backup, DEFAULT_SETTINGS2.auto_backup),
    backup_before_launch: parseBoolean(input.backup_before_launch, DEFAULT_SETTINGS2.backup_before_launch),
    backup_compression_enabled: parseBoolean(input.backup_compression_enabled, DEFAULT_SETTINGS2.backup_compression_enabled),
    backup_compression_level: parseIntValue(input.backup_compression_level, DEFAULT_SETTINGS2.backup_compression_level, 1, 100),
    backup_skip_compression_once: parseBoolean(input.backup_skip_compression_once, DEFAULT_SETTINGS2.backup_skip_compression_once),
    max_backups_per_game: parseIntValue(input.max_backups_per_game, DEFAULT_SETTINGS2.max_backups_per_game, 1, 100),
    rawg_api_key: typeof input.rawg_api_key === "string" ? input.rawg_api_key : "",
    start_minimized_in_tray: parseBoolean(input.start_minimized_in_tray, DEFAULT_SETTINGS2.start_minimized_in_tray)
  };
}
function serializeAppSettings(settings) {
  return {
    theme: settings.theme,
    ludusavi_path: settings.ludusavi_path,
    backup_directory: settings.backup_directory,
    auto_backup: String(settings.auto_backup),
    backup_before_launch: String(settings.backup_before_launch),
    backup_compression_enabled: String(settings.backup_compression_enabled),
    backup_compression_level: String(clamp(settings.backup_compression_level, 1, 100)),
    backup_skip_compression_once: String(settings.backup_skip_compression_once),
    max_backups_per_game: String(clamp(settings.max_backups_per_game, 1, 100)),
    rawg_api_key: settings.rawg_api_key,
    start_minimized_in_tray: String(settings.start_minimized_in_tray)
  };
}
function shouldStartMinimizedInTray(settings) {
  return settings?.start_minimized_in_tray ?? DEFAULT_SETTINGS2.start_minimized_in_tray;
}

// electron/main/services/settings.ts
function mergeSettings(raw) {
  return normalizeAppSettings({
    ...getDefaultAppSettings(),
    ...raw
  });
}
function createSettingsService(repository) {
  return {
    async getAllSettings() {
      return mergeSettings(await repository.listSettings());
    },
    async updateSettings(settings) {
      await repository.upsertSettings(serializeAppSettings(normalizeAppSettings(settings)));
    },
    getSetting: (key) => repository.getSetting(key),
    setSetting: (key, value) => repository.upsertSetting(key, value),
    addScanDirectory: (path6) => repository.addScanDirectory(path6),
    getScanDirectories: () => repository.listScanDirectories(),
    removeScanDirectory: (path6) => repository.removeScanDirectory(path6),
    shouldStartMinimizedInTray
  };
}

// electron/main/services/helpers/date-range.ts
var UTC_DAY_MS = 24 * 60 * 60 * 1000;
function parseIsoDateOnly(value) {
  if (!value) {
    return null;
  }
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value.trim());
  if (!match) {
    return null;
  }
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const date = new Date(Date.UTC(year, month - 1, day));
  if (date.getUTCFullYear() !== year || date.getUTCMonth() !== month - 1 || date.getUTCDate() !== day) {
    return null;
  }
  return date;
}
function formatIsoDateOnly(value) {
  return [
    value.getUTCFullYear(),
    String(value.getUTCMonth() + 1).padStart(2, "0"),
    String(value.getUTCDate()).padStart(2, "0")
  ].join("-");
}
function addUtcDays(value, amount) {
  return new Date(value.getTime() + amount * UTC_DAY_MS);
}
function getUtcToday() {
  const now = new Date;
  return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()));
}
function normalizeDateRange(start, end, windowDays = 30) {
  const today = getUtcToday();
  let endDate = parseIsoDateOnly(end) ?? today;
  let startDate = parseIsoDateOnly(start) ?? addUtcDays(endDate, -(windowDays - 1));
  if (startDate > endDate) {
    [startDate, endDate] = [endDate, startDate];
  }
  return {
    startDate,
    endDate,
    rangeStart: formatIsoDateOnly(startDate),
    rangeEnd: formatIsoDateOnly(endDate)
  };
}
function enumerateUtcDates(startDate, endDate) {
  const dates = [];
  let cursor = new Date(startDate);
  while (cursor <= endDate) {
    dates.push(new Date(cursor));
    cursor = addUtcDays(cursor, 1);
  }
  return dates;
}

// electron/main/services/stats.ts
function createStatsService(repository) {
  return {
    async getPlaytimeStats(start, end) {
      const range = normalizeDateRange(start, end);
      const [dailyTotals, perGameTotals] = await Promise.all([
        repository.getDailyTotals(range.rangeStart, range.rangeEnd),
        repository.getPerGameTotals(range.rangeStart, range.rangeEnd)
      ]);
      const dailyMap = new Map(dailyTotals.map((entry) => [entry.date, entry.seconds]));
      const dailySeries = enumerateUtcDates(range.startDate, range.endDate).map((date) => {
        const dateKey = date.toISOString().slice(0, 10);
        return {
          date: dateKey,
          seconds: dailyMap.get(dateKey) ?? 0
        };
      });
      const orderedGameTotals = [...perGameTotals].sort((a, b) => b.seconds - a.seconds);
      return {
        range_start: range.rangeStart,
        range_end: range.rangeEnd,
        total_seconds: dailySeries.reduce((sum, entry) => sum + entry.seconds, 0),
        daily_totals: dailySeries,
        per_game_totals: orderedGameTotals
      };
    }
  };
}

// electron/main/services/helpers/electron.ts
import { app, screen } from "electron";
async function collectElectronGpuInfo() {
  if (!app.isReady()) {
    return [];
  }
  try {
    const info = await app.getGPUInfo("complete");
    const devices = Array.isArray(info.gpuDevice) ? info.gpuDevice : [];
    return devices.map((device, index) => ({
      name: typeof device.deviceString === "string" && device.deviceString || typeof device.vendorString === "string" && device.vendorString || `GPU ${index + 1}`,
      device_name: typeof device.deviceString === "string" && device.deviceString || typeof device.vendorString === "string" && device.vendorString || "",
      is_primary: Boolean(device.active ?? index === 0)
    }));
  } catch {
    return [];
  }
}
function collectElectronMonitorInfo() {
  try {
    const primaryId = screen.getPrimaryDisplay().id;
    return screen.getAllDisplays().map((display, index) => ({
      name: display.id === primaryId ? "Primary display" : `Display ${index + 1}`,
      device_name: String(display.id),
      width: display.size.width,
      height: display.size.height,
      refresh_rate: Math.round(display.displayFrequency ?? 0),
      is_primary: display.id === primaryId
    }));
  } catch {
    return [];
  }
}

// electron/main/services/helpers/disk.ts
import { execFile as execFile3 } from "node:child_process";
import { mkdtemp, open, rm, stat as stat3 } from "node:fs/promises";
import path6 from "node:path";
import { promisify as promisify3 } from "node:util";
var execFileAsync2 = promisify3(execFile3);
var TEST_FILE_SIZE = 128 * 1024 * 1024;
var CHUNK_SIZE = 4 * 1024 * 1024;
function toBigIntBytes(value) {
  if (typeof value === "number" && Number.isFinite(value)) {
    return Math.max(0, Math.trunc(value));
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number.parseInt(value, 10);
    return Number.isNaN(parsed) ? 0 : Math.max(0, parsed);
  }
  return 0;
}
function formatDiskKind(value) {
  const driveType = toBigIntBytes(value);
  switch (driveType) {
    case 2:
      return "Removable";
    case 3:
      return "Fixed";
    case 4:
      return "Network";
    case 5:
      return "CD-ROM";
    case 6:
      return "RAM";
    default:
      return "Unknown";
  }
}
async function collectWindowsDisks() {
  try {
    const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
Get-CimInstance Win32_LogicalDisk |
  Select-Object DeviceID, VolumeName, FileSystem, Size, FreeSpace, DriveType |
  ConvertTo-Json -Depth 4 -Compress
`.trim();
    const { stdout } = await execFileAsync2("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script], { maxBuffer: 16 * 1024 * 1024, windowsHide: true });
    const text = stdout.toString("utf8").trim();
    const rows = text ? JSON.parse(text) : [];
    const list = Array.isArray(rows) ? rows : [rows];
    return list.map((row) => {
      if (!row || typeof row !== "object") {
        return null;
      }
      const entry = row;
      const mountPoint = typeof entry.DeviceID === "string" ? entry.DeviceID : "";
      if (!mountPoint) {
        return null;
      }
      const fileSystem = typeof entry.FileSystem === "string" && entry.FileSystem ? entry.FileSystem : "Unknown";
      const volumeName = typeof entry.VolumeName === "string" && entry.VolumeName ? entry.VolumeName : null;
      return {
        name: volumeName ?? mountPoint,
        mount_point: mountPoint,
        file_system: fileSystem,
        total_bytes: toBigIntBytes(entry.Size),
        available_bytes: toBigIntBytes(entry.FreeSpace),
        kind: formatDiskKind(entry.DriveType),
        is_removable: toBigIntBytes(entry.DriveType) === 2,
        model: volumeName,
        media_type: null
      };
    }).filter((entry) => Boolean(entry)).sort((a, b) => a.mount_point.localeCompare(b.mount_point));
  } catch {
    return [];
  }
}
async function getFilesystemType(mountPoint) {
  try {
    const args = process.platform === "darwin" ? ["-f", "%T", mountPoint] : ["-f", "-c", "%T", mountPoint];
    const { stdout } = await execFileAsync2("stat", args, {
      maxBuffer: 1024 * 1024
    });
    const value = stdout.toString("utf8").trim();
    return value || "Unknown";
  } catch {
    return "Unknown";
  }
}
async function collectPosixDisks() {
  try {
    const { stdout } = await execFileAsync2("df", ["-Pk"], {
      maxBuffer: 16 * 1024 * 1024
    });
    const lines = stdout.toString("utf8").split(/\r?\n/).slice(1).map((line) => line.trim()).filter(Boolean);
    const rows = lines.map((line) => {
      const match = /^(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+\d+%\s+(.+)$/.exec(line);
      if (!match) {
        return null;
      }
      return {
        source: match[1],
        totalBlocks: Number(match[2]),
        availableBlocks: Number(match[4]),
        mountPoint: match[5]
      };
    }).filter((row) => Boolean(row));
    const fileSystems = await Promise.all(rows.map(async (row) => ({
      mountPoint: row.mountPoint,
      fileSystem: await getFilesystemType(row.mountPoint)
    })));
    const fileSystemMap = new Map(fileSystems.map((entry) => [entry.mountPoint, entry.fileSystem]));
    return rows.map((row) => {
      const mountPoint = row.mountPoint;
      const fsType = fileSystemMap.get(mountPoint) ?? "Unknown";
      const source = row.source;
      const kind = source.startsWith("/dev/") ? "Fixed" : source.startsWith("//") || source.startsWith("\\\\") ? "Network" : "Unknown";
      return {
        name: source,
        mount_point: mountPoint,
        file_system: fsType,
        total_bytes: row.totalBlocks * 1024,
        available_bytes: row.availableBlocks * 1024,
        kind,
        is_removable: false,
        model: null,
        media_type: null
      };
    }).sort((a, b) => a.mount_point.localeCompare(b.mount_point));
  } catch {
    return [];
  }
}
async function collectDiskInfos() {
  if (process.platform === "win32") {
    return collectWindowsDisks();
  }
  return collectPosixDisks();
}
async function testDiskSpeed(mountPoint, sizeBytes = TEST_FILE_SIZE) {
  if (!mountPoint.trim()) {
    throw new Error("Empty mount point");
  }
  const resolvedMount = path6.resolve(mountPoint);
  const statInfo = await stat3(resolvedMount).catch(() => null);
  if (!statInfo || !statInfo.isDirectory()) {
    throw new Error("Invalid mount point");
  }
  const testDir = await mkdtemp(path6.join(resolvedMount, "arrancador-speedtest-"));
  const testFile = path6.join(testDir, "speedtest.bin");
  const buffer = Buffer.alloc(CHUNK_SIZE, 165);
  const startedWrite = performance.now();
  try {
    const handle = await open(testFile, "w");
    try {
      let remaining = sizeBytes;
      while (remaining > 0) {
        const toWrite = Math.min(remaining, CHUNK_SIZE);
        await handle.write(buffer.subarray(0, toWrite));
        remaining -= toWrite;
      }
      await handle.sync();
    } finally {
      await handle.close();
    }
    const elapsedWriteMs = Math.max(1, Math.round(performance.now() - startedWrite));
    const startedRead = performance.now();
    const readBuffer = Buffer.alloc(CHUNK_SIZE);
    const reader = await open(testFile, "r");
    try {
      while (true) {
        const result = await reader.read(readBuffer, 0, readBuffer.length, null);
        if (result.bytesRead <= 0) {
          break;
        }
      }
    } finally {
      await reader.close();
    }
    const elapsedReadMs = Math.max(1, Math.round(performance.now() - startedRead));
    const sizeMb = sizeBytes / 1048576;
    return {
      mount_point: mountPoint,
      size_bytes: sizeBytes,
      write_mbps: sizeMb / (elapsedWriteMs / 1000),
      read_mbps: sizeMb / (elapsedReadMs / 1000),
      elapsed_write_ms: elapsedWriteMs,
      elapsed_read_ms: elapsedReadMs
    };
  } finally {
    await rm(testDir, { recursive: true, force: true }).catch(() => {
      return;
    });
  }
}

// electron/main/services/helpers/system.ts
import os2 from "node:os";
import { readFile } from "node:fs/promises";
async function getOsName() {
  if (process.platform === "linux") {
    try {
      const text = await readFile("/etc/os-release", "utf8");
      const pretty = /^(?:PRETTY_NAME|NAME)="([^"]+)"/m.exec(text)?.[1];
      if (pretty) {
        return pretty;
      }
    } catch {}
  }
  if (process.platform === "darwin") {
    return "macOS";
  }
  if (process.platform === "win32") {
    return "Windows";
  }
  return os2.type();
}
async function getOsVersion() {
  if (process.platform === "linux") {
    try {
      const text = await readFile("/etc/os-release", "utf8");
      const version = /^VERSION(?:_ID)?="([^"]+)"/m.exec(text)?.[1];
      if (version) {
        return version;
      }
    } catch {}
  }
  return os2.release();
}
function getCpuInfo() {
  const cpus = os2.cpus();
  const first = cpus[0];
  const frequency = cpus.length ? Math.round(cpus.reduce((sum, cpu) => sum + cpu.speed, 0) / cpus.length) : 0;
  const logicalCoreFallback = cpus.length || 1;
  return {
    brand: first?.model ?? "Unknown",
    vendor_id: "",
    frequency_mhz: frequency,
    physical_cores: null,
    logical_cores: Math.max(1, os2.availableParallelism?.() ?? logicalCoreFallback)
  };
}
function getMemoryInfo() {
  return {
    total_bytes: os2.totalmem(),
    used_bytes: os2.totalmem() - os2.freemem(),
    free_bytes: os2.freemem(),
    available_bytes: os2.freemem(),
    total_swap_bytes: 0,
    used_swap_bytes: 0
  };
}
function getSystemIdentity() {
  const uptimeSeconds = Math.floor(os2.uptime());
  return {
    hostname: os2.hostname() || null,
    os_name: null,
    os_version: null,
    kernel_version: os2.release() || null,
    uptime_seconds: uptimeSeconds,
    boot_time: Math.max(0, Math.floor(Date.now() / 1000) - uptimeSeconds),
    arch: process.arch
  };
}

// electron/main/services/system.ts
async function getSystemInfo(options = {}) {
  const identity = getSystemIdentity();
  const [osName, osVersion, disks, gpus, monitors] = await Promise.all([
    getOsName(),
    getOsVersion(),
    options.getDiskInfo?.() ?? collectDiskInfos(),
    options.getGpuInfo?.() ?? collectElectronGpuInfo(),
    options.getMonitorInfo?.() ?? collectElectronMonitorInfo()
  ]);
  return {
    ...identity,
    os_name: osName,
    os_version: osVersion,
    cpu: getCpuInfo(),
    memory: getMemoryInfo(),
    disks,
    gpus,
    monitors
  };
}
async function testDiskSpeed2(mountPoint) {
  return testDiskSpeed(mountPoint);
}
function createSystemService(options = {}) {
  return {
    getSystemInfo: () => getSystemInfo(options),
    testDiskSpeed: testDiskSpeed2
  };
}

// electron/main/services/backup/index.ts
import { randomUUID as randomUUID3 } from "node:crypto";
import { stat as stat6 } from "node:fs/promises";
import path14 from "node:path";

// electron/main/services/backup/archive.ts
import { execFile as execFile4 } from "node:child_process";
import { readFile as readFile2, writeFile } from "node:fs/promises";
import path8 from "node:path";
import { promisify as promisify4 } from "node:util";

// electron/main/services/backup/utils.ts
import { access, lstat, mkdir, mkdtemp as mkdtemp2, readdir, rm as rm2, stat as stat4 } from "node:fs/promises";
import { tmpdir } from "node:os";
import path7 from "node:path";
async function pathExists(targetPath) {
  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}
async function isDirectory(targetPath) {
  try {
    return (await stat4(targetPath)).isDirectory();
  } catch {
    return false;
  }
}
async function isFile(targetPath) {
  try {
    return (await stat4(targetPath)).isFile();
  } catch {
    return false;
  }
}
async function removeBackupArtifact(targetPath) {
  if (!await pathExists(targetPath)) {
    return;
  }
  await rm2(targetPath, { recursive: true, force: true });
}
async function buildUniqueBackupPath(backupRoot, timestamp, useCompression) {
  let attempt = 0;
  while (true) {
    const suffix = attempt === 0 ? timestamp : `${timestamp}_${attempt}`;
    const candidate = useCompression ? path7.join(backupRoot, `${suffix}.sqoba.zip`) : path7.join(backupRoot, suffix);
    if (!await pathExists(candidate)) {
      return candidate;
    }
    attempt += 1;
  }
}
async function createTempDir(prefix) {
  return mkdtemp2(path7.join(tmpdir(), prefix));
}
async function mapLimit(items, limit, worker) {
  const concurrency = Math.max(1, Math.trunc(limit) || 1);
  const results = new Array(items.length);
  let nextIndex = 0;
  const runners = Array.from({ length: Math.min(concurrency, items.length) }, async () => {
    while (true) {
      const current = nextIndex;
      nextIndex += 1;
      if (current >= items.length) {
        return;
      }
      results[current] = await worker(items[current], current);
    }
  });
  await Promise.all(runners);
  return results;
}
async function readDirectoryEntries(targetPath) {
  return readdir(targetPath, { withFileTypes: true });
}
async function ensureDir(targetPath) {
  await mkdir(targetPath, { recursive: true });
}
async function statPath(targetPath) {
  return stat4(targetPath);
}

// electron/main/services/backup/archive.ts
var execFileAsync3 = promisify4(execFile4);
var BACKUP_MANIFEST_NAMES = ["__sqoba_manifest.json", "__arrancador_manifest.json"];
var BACKUP_README_NAME = "__sqoba_readme.txt";
var MANIFEST_VERSION = 2;
function escapePowerShellString(value) {
  return `'${value.replace(/'/g, "''")}'`;
}
async function runPowerShellScript(script) {
  if (process.platform !== "win32") {
    throw new Error("ZIP backups require Windows PowerShell");
  }
  const args = ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script];
  try {
    await execFileAsync3("powershell.exe", args, {
      windowsHide: true,
      maxBuffer: 10 * 1024 * 1024
    });
  } catch (error) {
    if (error.code !== "ENOENT") {
      throw error;
    }
    await execFileAsync3("pwsh", args, {
      windowsHide: true,
      maxBuffer: 10 * 1024 * 1024
    });
  }
}
function compressionMode(level) {
  return (level ?? 60) >= 50 ? "optimal" : "fastest";
}
function buildBackupManifest(entries) {
  return {
    version: MANIFEST_VERSION,
    files: entries
  };
}
async function writeBackupManifestToDirectory(destination, manifest) {
  await writeFile(path8.join(destination, BACKUP_MANIFEST_NAMES[0]), JSON.stringify(manifest, null, 2), "utf8");
}
async function writeBackupReadmeToDirectory(destination) {
  const readme = [
    "SQOBA backup format",
    "",
    "This folder contains raw save files plus a manifest.",
    `- ${BACKUP_MANIFEST_NAMES[0]}: list of files and original paths`,
    "- files/: backed up files in the same structure as the saves",
    "",
    "To restore manually:",
    `1) Open ${BACKUP_MANIFEST_NAMES[0]}`,
    "2) For each entry, copy files/<path> to original_path",
    ""
  ].join(`
`);
  await writeFile(path8.join(destination, BACKUP_README_NAME), readme, "utf8");
}
async function compressBackupDirectory(destinationZip, sourceDir, level) {
  await ensureDir(path8.dirname(destinationZip));
  await removeBackupArtifact(destinationZip);
  const compression = compressionMode(level);
  const sourcePattern = path8.join(sourceDir, "*");
  const script = [
    `Compress-Archive -Path ${escapePowerShellString(sourcePattern)}`,
    `-DestinationPath ${escapePowerShellString(destinationZip)}`,
    `-CompressionLevel ${compression === "optimal" ? "Optimal" : "Fastest"}`,
    "-Force"
  ].join(" ");
  await runPowerShellScript(script);
}
async function expandBackupArchive(archivePath, destinationDir) {
  await ensureDir(destinationDir);
  const script = [
    `Expand-Archive -Path ${escapePowerShellString(archivePath)}`,
    `-DestinationPath ${escapePowerShellString(destinationDir)}`,
    "-Force"
  ].join(" ");
  await runPowerShellScript(script);
}
async function readBackupManifestFromDirectory(backupRoot) {
  for (const manifestName of BACKUP_MANIFEST_NAMES) {
    const manifestPath = path8.join(backupRoot, manifestName);
    if (!await isFile(manifestPath)) {
      continue;
    }
    const text = await readFile2(manifestPath, "utf8");
    return JSON.parse(text);
  }
  return null;
}
async function loadBackupManifest(backupPath) {
  if (await isDirectory(backupPath)) {
    return readBackupManifestFromDirectory(backupPath);
  }
  if (!await isFile(backupPath)) {
    return null;
  }
  if (path8.extname(backupPath).toLowerCase() !== ".zip") {
    return null;
  }
  const tempDir = await createTempDir("arrancador-manifest-");
  try {
    await expandBackupArchive(backupPath, tempDir);
    return readBackupManifestFromDirectory(tempDir);
  } finally {
    await removeBackupArtifact(tempDir);
  }
}

// electron/main/services/backup/copy.ts
import { copyFile, stat as stat5 } from "node:fs/promises";
import os3 from "node:os";
import path9 from "node:path";
function buildBackupRelPath(rootLabel, relativePath) {
  const rel = relativePath.replaceAll("\\", "/").replace(/^\/+/, "");
  if (!rel) {
    return `files/${rootLabel}/file`;
  }
  return `files/${rootLabel}/${rel}`;
}
async function fileMtime(filePath) {
  try {
    const metadata = await stat5(filePath);
    const mtime = metadata.mtimeMs;
    return Number.isFinite(mtime) ? Math.trunc(mtime) : null;
  } catch {
    return null;
  }
}
async function copyDiscoveryToDirectory(destination, discovery, options = {}) {
  await ensureDir(destination);
  const concurrency = options.concurrency ?? Math.max(1, Math.min(8, os3.cpus().length || 1));
  const entries = [];
  let completed = 0;
  let totalBytes = 0;
  await mapLimit(discovery.files, concurrency, async (file, index) => {
    const backupPath = buildBackupRelPath(file.rootLabel, file.relativePath);
    const targetPath = path9.join(destination, backupPath);
    await ensureDir(path9.dirname(targetPath));
    await copyFile(file.path, targetPath);
    const size = file.size;
    const entry = {
      backupPath,
      originalPath: file.path,
      size,
      mtime: await fileMtime(file.path)
    };
    entries[index] = entry;
    totalBytes += size;
    completed += 1;
    if (options.onProgress && (completed === discovery.files.length || completed % 50 === 0)) {
      await options.onProgress({
        stage: "copy",
        current: file.path,
        done: completed,
        total: discovery.files.length
      });
    }
    return entry;
  });
  const manifest = buildBackupManifest(entries);
  await writeBackupManifestToDirectory(destination, manifest);
  await writeBackupReadmeToDirectory(destination);
  return {
    manifest,
    totalBytes
  };
}

// electron/main/services/backup/list.ts
import path10 from "node:path";
var WIN_PATH = path10.win32;
function parseBackupTimestamp(name) {
  const trimmed = name.replace(/(\.sqoba\.zip|\.zip)$/i, "");
  const match = trimmed.match(/^(\d{6})_(\d{2})(\d{2})(\d{4})(?:_\d+)?$/);
  if (!match) {
    return null;
  }
  const [, time, day, month, year] = match;
  const hours = Number.parseInt(time.slice(0, 2), 10);
  const minutes = Number.parseInt(time.slice(2, 4), 10);
  const seconds = Number.parseInt(time.slice(4, 6), 10);
  const dt = new Date(Number.parseInt(year, 10), Number.parseInt(month, 10) - 1, Number.parseInt(day, 10), hours, minutes, seconds, 0);
  return Number.isNaN(dt.getTime()) ? null : dt;
}
function backupEntryTimestamp(backupPath) {
  const name = path10.basename(backupPath);
  const parsed = parseBackupTimestamp(name);
  if (parsed) {
    return parsed;
  }
  return new Date;
}
function sanitizeFolderName(name) {
  return name.replace(/[<>:"/\\|?*]/g, "").trim();
}
async function findBackupGameDirsAsync(backupRoot, gameName, gameYear) {
  const base = sanitizeFolderName(gameName).toLowerCase();
  if (!base || !await pathExists(backupRoot)) {
    return [];
  }
  const expectedYear = gameYear?.trim();
  const expectedParens = expectedYear ? `${base} (${expectedYear.toLowerCase()})` : null;
  const out = [];
  const entries = await readDirectoryEntries(backupRoot);
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const name = entry.name.toLowerCase();
    const matchesDefault = name === base || name.startsWith(`${base}-`);
    const matchesParens = expectedParens ? name === expectedParens : false;
    if (matchesDefault || matchesParens) {
      out.push(path10.join(backupRoot, entry.name));
    }
  }
  return out;
}
function parseBackupRelative(backupPath) {
  const parts = backupPath.split("/").filter(Boolean);
  if (parts.length < 3 || parts[0] !== "files") {
    return null;
  }
  return [parts[1], parts.slice(2).join("/")];
}
function stripSuffixPath(fullPath, suffix) {
  const normalizedFull = WIN_PATH.normalize(fullPath).replace(/[\\/]+$/, "");
  const normalizedSuffix = WIN_PATH.normalize(suffix).replace(/^[\\/]+|[\\/]+$/g, "");
  const lowerFull = normalizedFull.toLowerCase();
  const lowerSuffix = normalizedSuffix.toLowerCase();
  if (lowerFull === lowerSuffix) {
    return "";
  }
  const marker = `\\${lowerSuffix}`;
  if (!lowerFull.endsWith(marker)) {
    return null;
  }
  return normalizedFull.slice(0, normalizedFull.length - marker.length);
}
function deriveSaveRootFromManifest(manifest) {
  const totals = new Map;
  const roots = new Map;
  for (const entry of manifest.files) {
    const parsed = parseBackupRelative(entry.backupPath);
    if (!parsed) {
      continue;
    }
    const [rootLabel, rel] = parsed;
    const originalPath = entry.originalPath;
    const root = stripSuffixPath(originalPath, rel) ?? path10.dirname(originalPath);
    totals.set(rootLabel, (totals.get(rootLabel) ?? 0) + entry.size);
    if (!roots.has(rootLabel)) {
      roots.set(rootLabel, root);
    }
  }
  if (totals.size === 0) {
    const first = manifest.files[0];
    return first ? path10.dirname(first.originalPath) : null;
  }
  let bestLabel = null;
  let bestSize = -1;
  for (const [label, size] of totals.entries()) {
    if (size > bestSize) {
      bestLabel = label;
      bestSize = size;
    }
  }
  return bestLabel ? roots.get(bestLabel) ?? null : null;
}
async function backupSummaryFromArtifact(backupPath) {
  const manifest = await loadBackupManifest(backupPath);
  if (!manifest) {
    return null;
  }
  const stat6 = await statPath(backupPath).catch(() => null);
  const createdAt = backupEntryTimestamp(backupPath).toISOString();
  const kind = backupPath.toLowerCase().endsWith(".zip") ? "zip" : "directory";
  const saveRoot = deriveSaveRootFromManifest(manifest);
  const backupSize = manifest.files.reduce((sum, entry) => sum + entry.size, 0);
  return {
    id: backupPath,
    backupPath,
    backupSize,
    createdAt: stat6?.mtime.toISOString() ?? createdAt,
    isAuto: false,
    notes: null,
    kind,
    saveRoot
  };
}
async function listBackups(input) {
  const gameDirs = await findBackupGameDirsAsync(input.backupRoot, input.gameName, input.gameYear ?? null);
  const out = [];
  for (const gameDir of gameDirs) {
    const entries = await readDirectoryEntries(gameDir).catch(() => []);
    for (const entry of entries) {
      if (!entry.isDirectory() && !entry.name.toLowerCase().endsWith(".zip")) {
        continue;
      }
      const backupPath = path10.join(gameDir, entry.name);
      const summary = await backupSummaryFromArtifact(backupPath);
      if (summary) {
        out.push(summary);
      }
    }
  }
  out.sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt));
  return out;
}
async function pruneBackupsByLimit(input) {
  const backups = await listBackups({
    backupRoot: input.backupRoot,
    gameName: input.gameName,
    gameYear: input.gameYear ?? null
  });
  if (backups.length <= input.maxBackups) {
    return;
  }
  const toDelete = backups.slice(input.maxBackups);
  for (const backup of toDelete) {
    await removeBackupArtifact(backup.backupPath);
  }
}

// electron/main/services/backup/restore.ts
import { copyFile as copyFile2, readFile as readFile3 } from "node:fs/promises";
import path11 from "node:path";
var WIN_PATH2 = path11.win32;
function validateBackupRelPath(rel) {
  const parts = rel.split("/").filter(Boolean);
  if (parts.length === 0) {
    throw new Error(`Invalid backup path in manifest: ${rel}`);
  }
  for (const part of parts) {
    if (part === "." || part === "..") {
      throw new Error(`Invalid backup path in manifest: ${rel}`);
    }
    if (WIN_PATH2.isAbsolute(part) || part.includes(":")) {
      throw new Error(`Invalid backup path in manifest: ${rel}`);
    }
  }
}
function pathFromBackupRel(rel) {
  const parts = rel.split("/").filter(Boolean);
  return path11.join(...parts);
}
function validateRestoreTargetPath(original) {
  const rawParts = original.split(/[\\/]+/);
  if (rawParts.some((part) => part === "." || part === "..")) {
    throw new Error(`Invalid restore target path in manifest: ${original}`);
  }
  const target = WIN_PATH2.normalize(original);
  if (!WIN_PATH2.isAbsolute(target)) {
    throw new Error(`Invalid restore target path in manifest: ${original}`);
  }
  return target;
}
function splitDriveForRestore(original, inverseDrives) {
  const match = original.match(/^([A-Za-z]):[\\/](.*)$/);
  if (match) {
    const letter = match[1].toUpperCase();
    const rest = match[2].replaceAll("\\", "/");
    const prefix = `${letter}:`;
    return {
      sourceRoot: inverseDrives.get(prefix) ?? `drive-${letter}`,
      relative: rest
    };
  }
  return {
    sourceRoot: "drive-0",
    relative: original.replaceAll("\\", "/")
  };
}
function stripQuotes(input) {
  return input.replace(/^['"]|['"]$/g, "");
}
function normalizeDrivePrefix(value) {
  const trimmed = stripQuotes(value.trim()).replaceAll("/", "\\");
  const match = trimmed.match(/^([A-Za-z]):/);
  return match ? `${match[1].toUpperCase()}:` : trimmed.replace(/[\\/]+$/, "");
}
function parseLegacyMapping(text) {
  const drives = new Map;
  const backups = [];
  let section = "root";
  let currentBackup = null;
  let inFiles = false;
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.replace(/\t/g, "    ");
    const trimmed = line.trim();
    if (trimmed === "drives:") {
      section = "drives";
      inFiles = false;
      continue;
    }
    if (trimmed === "backups:") {
      section = "backups";
      inFiles = false;
      continue;
    }
    if (section === "drives") {
      const match = line.match(/^\s{2}([^:]+):\s*(.+)\s*$/);
      if (match) {
        drives.set(stripQuotes(match[1].trim()), normalizeDrivePrefix(match[2].trim()));
      }
      continue;
    }
    if (section === "backups") {
      if (/^\s{2}-\s+/.test(line)) {
        currentBackup = { files: [] };
        backups.push(currentBackup);
        inFiles = false;
        continue;
      }
      if (/^\s{4}files:\s*$/.test(line)) {
        inFiles = true;
        continue;
      }
      if (inFiles && currentBackup) {
        const match = line.match(/^\s{6,}(?:"([^"]+)"|'([^']+)'|([^:]+)):\s*$/);
        if (match) {
          const value = match[1] ?? match[2] ?? match[3];
          if (value) {
            currentBackup.files.push(stripQuotes(value.trim()));
          }
        }
      }
    }
  }
  return {
    drives,
    files: backups.at(-1)?.files ?? []
  };
}
async function restoreFromEntries(entries, onProgress) {
  const total = entries.length;
  let completed = 0;
  await mapLimit(entries, Math.min(8, total || 1), async ({ source, target }) => {
    await ensureDir(path11.dirname(target));
    await copyFile2(source, target);
    completed += 1;
    if (onProgress && (completed === total || completed % 50 === 0)) {
      await onProgress({
        stage: "restore",
        current: target,
        done: completed,
        total
      });
    }
  });
}
async function restoreManifestDirectory(backupRoot, manifest, onProgress) {
  const entries = manifest.files.map((entry) => {
    validateBackupRelPath(entry.backupPath);
    const source = path11.join(backupRoot, pathFromBackupRel(entry.backupPath));
    const target = validateRestoreTargetPath(entry.originalPath);
    return { source, target };
  });
  for (const entry of entries) {
    if (!await pathExists(entry.source)) {
      throw new Error(`Backup file is missing: ${entry.source}`);
    }
  }
  await restoreFromEntries(entries, onProgress);
}
async function restoreLegacyMapping(backupRoot, mappingPath, onProgress) {
  const mappingText = await readFile3(mappingPath, "utf8");
  const mapping = parseLegacyMapping(mappingText);
  const entries = mapping.files.map((originalPath) => {
    const { sourceRoot, relative } = splitDriveForRestore(originalPath, mapping.drives);
    const sourceRel = `${sourceRoot}/${relative}`.replace(/\/+/g, "/");
    validateBackupRelPath(sourceRel);
    const source = path11.join(backupRoot, pathFromBackupRel(sourceRel));
    const target = validateRestoreTargetPath(originalPath.replaceAll("/", "\\"));
    return { source, target };
  });
  for (const entry of entries) {
    if (!await pathExists(entry.source)) {
      throw new Error(`Backup file is missing: ${entry.source}`);
    }
  }
  await restoreFromEntries(entries, onProgress);
}
async function restoreBackupDirectory(backupRoot, onProgress) {
  const manifest = await readBackupManifestFromDirectory(backupRoot);
  if (manifest) {
    await restoreManifestDirectory(backupRoot, manifest, onProgress);
    return;
  }
  const mappingPath = path11.join(backupRoot, "mapping.yaml");
  if (await isFile(mappingPath)) {
    await restoreLegacyMapping(backupRoot, mappingPath, onProgress);
    return;
  }
  throw new Error("Backup manifest is missing");
}
async function restoreBackupArtifact(backupPath, onProgress) {
  if (await isDirectory(backupPath)) {
    await restoreBackupDirectory(backupPath, onProgress);
    return;
  }
  if (!await isFile(backupPath)) {
    throw new Error(`Backup does not exist: ${backupPath}`);
  }
  if (!backupPath.toLowerCase().endsWith(".zip")) {
    throw new Error(`Unsupported backup artifact: ${backupPath}`);
  }
  const tempDir = await createTempDir("arrancador-restore-");
  try {
    await expandBackupArchive(backupPath, tempDir);
    await restoreBackupDirectory(tempDir, onProgress);
  } finally {
    await removeBackupArtifact(tempDir);
  }
}

// electron/main/services/backup/save-locator.ts
import { execFile as execFile5 } from "node:child_process";
import { readFile as readFile5 } from "node:fs/promises";
import os4 from "node:os";
import path13 from "node:path";
import { promisify as promisify5 } from "node:util";

// electron/main/services/backup/manifest.ts
import { readFile as readFile4, writeFile as writeFile2 } from "node:fs/promises";
import path12 from "node:path";
var NORMALIZE_RE = /[^a-z0-9]+/g;
function normalizeName(name) {
  const stopWords = new Set([
    "the",
    "a",
    "an",
    "edition",
    "definitive",
    "remastered",
    "goty",
    "game",
    "of",
    "year",
    "ultimate",
    "complete",
    "collection",
    "bundle",
    "deluxe",
    "enhanced",
    "hd"
  ]);
  const cleaned = name.toLowerCase().replace(NORMALIZE_RE, " ");
  const tokens = cleaned.split(/\s+/).map((token) => token.trim()).filter((token) => token.length > 0 && !stopWords.has(token));
  return tokens.join(" ");
}
function similarityScore(a, b) {
  if (!a || !b) {
    return 0;
  }
  if (a === b) {
    return 1;
  }
  if (a.includes(b) || b.includes(a)) {
    return 0.9;
  }
  const setA = new Set(a.split(/\s+/).filter(Boolean));
  const setB = new Set(b.split(/\s+/).filter(Boolean));
  if (setA.size === 0 || setB.size === 0) {
    return 0;
  }
  let intersection = 0;
  for (const value of setA) {
    if (setB.has(value)) {
      intersection += 1;
    }
  }
  const union = new Set([...setA, ...setB]).size;
  return union === 0 ? 0 : intersection / union;
}
function findGameEntry(manifest, name) {
  if (!manifest) {
    return null;
  }
  const exact = manifest.games[name];
  if (exact) {
    return [name, exact];
  }
  const normalizedTarget = normalizeName(name);
  const normalizedExact = new Map;
  const normalizedKeys = [];
  for (const key of Object.keys(manifest.games)) {
    const normalized = normalizeName(key);
    if (!normalizedExact.has(normalized)) {
      normalizedExact.set(normalized, key);
    }
    normalizedKeys.push([key, normalized]);
  }
  const directKey = normalizedExact.get(normalizedTarget);
  if (directKey) {
    return [directKey, manifest.games[directKey]];
  }
  let bestKey = null;
  let bestScore = 0;
  for (const [key, normalized] of normalizedKeys) {
    const score = similarityScore(normalizedTarget, normalized);
    if (score > bestScore) {
      bestKey = key;
      bestScore = score;
    }
  }
  if (bestKey && bestScore >= 0.6) {
    return [bestKey, manifest.games[bestKey]];
  }
  return null;
}
async function loadGameManifestCache(cachePath) {
  try {
    const text = await readFile4(cachePath, "utf8");
    const parsed = JSON.parse(text);
    if (!parsed || typeof parsed !== "object" || !parsed.games) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}
function manifestCachePath(baseDir) {
  return path12.join(baseDir, "sqoba_manifest.json");
}

// electron/main/services/backup/save-locator.ts
var execFileAsync4 = promisify5(execFile5);
var SAVE_PATH_GAME_TOKEN = "{PATHTOGAME}";
var WIN_PATH3 = path13.win32;
async function queryRegistryInstallPath() {
  if (process.platform !== "win32") {
    return null;
  }
  const queries = [
    "HKLM\\SOFTWARE\\Wow6432Node\\Valve\\Steam",
    "HKLM\\SOFTWARE\\Valve\\Steam"
  ];
  for (const key of queries) {
    try {
      const { stdout } = await execFileAsync4("reg", ["query", key, "/v", "InstallPath"], {
        windowsHide: true,
        maxBuffer: 1024 * 1024
      });
      const lines = stdout.split(/\r?\n/);
      for (const line of lines) {
        const match = line.match(/InstallPath\s+REG_SZ\s+(.+)$/i);
        if (match?.[1]) {
          const value = match[1].trim();
          if (value) {
            return value;
          }
        }
      }
    } catch {}
  }
  return null;
}
async function findSteamPath() {
  if (process.platform !== "win32") {
    return null;
  }
  const registry = await queryRegistryInstallPath();
  if (registry && await pathExists(registry)) {
    return registry;
  }
  const candidates = [
    process.env["ProgramFiles(x86)"] ? path13.join(process.env["ProgramFiles(x86)"], "Steam") : null,
    process.env.ProgramFiles ? path13.join(process.env.ProgramFiles, "Steam") : null,
    "C:\\Program Files (x86)\\Steam",
    "C:\\Program Files\\Steam"
  ].filter((candidate) => Boolean(candidate));
  for (const candidate of candidates) {
    if (await pathExists(candidate)) {
      return candidate;
    }
  }
  return null;
}
async function createContext() {
  const home = os4.homedir() || null;
  const documents = process.env.USERPROFILE ? path13.join(process.env.USERPROFILE, "Documents") : null;
  const appdata = process.env.APPDATA || null;
  const localAppData = process.env.LOCALAPPDATA || null;
  const localLow = localAppData ? path13.join(path13.dirname(localAppData), "LocalLow") : null;
  const savedGames = home ? path13.join(home, "Saved Games") : null;
  const publicPath = process.env.PUBLIC || null;
  const publicDocuments = publicPath ? path13.join(publicPath, "Documents") : null;
  const programData = process.env.ProgramData || null;
  const steam = await findSteamPath();
  const steamUserData = steam ? path13.join(steam, "userdata") : null;
  return {
    home,
    documents,
    appdata,
    localAppData,
    localLow,
    savedGames,
    publicPath,
    publicDocuments,
    programData,
    steam,
    steamUserData
  };
}
function sanitizeName(name) {
  return name.replace(/[<>:"/\\|?*]/g, "").trim().replace(/\s+/g, " ");
}
function candidateNames(gameName) {
  const out = new Set;
  const base = sanitizeName(gameName);
  const normalized = sanitizeName(gameName.toLowerCase());
  const collapsed = base.replace(/\s+/g, "");
  for (const value of [base, normalized, collapsed]) {
    if (value.trim()) {
      out.add(value.trim());
    }
  }
  return [...out];
}
function replaceToken(base, token, value, missing) {
  if (!base.includes(token)) {
    return base;
  }
  if (!value) {
    missing.value = true;
    return base;
  }
  return base.replaceAll(token, value);
}
function expandEnvVars(input) {
  return input.replace(/%([^%]+)%/g, (match, key) => process.env[key] ?? match);
}
function expandTilde(input, home) {
  if (!home || !input.startsWith("~")) {
    return input;
  }
  return `${home}${input.slice(1)}`;
}
function hasGlobChars(segment) {
  return /[*?]/.test(segment);
}
function globToRegExp(segment) {
  const escaped = segment.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  const regex = escaped.replace(/\*/g, ".*").replace(/\?/g, ".");
  return new RegExp(`^${regex}$`, "i");
}
async function expandGlobPattern(pattern) {
  const normalized = pattern.replaceAll("\\", "/");
  const root = WIN_PATH3.parse(pattern).root || "";
  const rootNormalized = root.replaceAll("\\", "/");
  const rest = normalized.startsWith(rootNormalized) ? normalized.slice(rootNormalized.length) : normalized;
  const segments = rest.split("/").filter(Boolean);
  let current = [root || pattern.slice(0, 2) || ""].filter(Boolean);
  if (current.length === 0) {
    current = [path13.parse(pattern).root || ""].filter(Boolean);
  }
  for (const segment of segments) {
    const next = [];
    for (const base of current) {
      const candidateBase = base || root || "";
      if (!candidateBase) {
        continue;
      }
      if (!hasGlobChars(segment)) {
        const candidate = WIN_PATH3.join(candidateBase, segment);
        if (await pathExists(candidate)) {
          next.push(candidate);
        }
        continue;
      }
      if (!await pathExists(candidateBase) || !await isDirectory(candidateBase)) {
        continue;
      }
      const entries = await readDirectoryEntries(candidateBase);
      const matcher = globToRegExp(segment);
      for (const entry of entries) {
        if (matcher.test(entry.name)) {
          next.push(WIN_PATH3.join(candidateBase, entry.name));
        }
      }
    }
    current = next;
    if (current.length === 0) {
      break;
    }
  }
  return current;
}
async function resolvePath(rawPath, context) {
  let pathText = rawPath;
  const missing = { value: false };
  pathText = replaceToken(pathText, "<home>", context.home, missing);
  pathText = replaceToken(pathText, "<winDocuments>", context.documents, missing);
  pathText = replaceToken(pathText, "<documents>", context.documents, missing);
  pathText = replaceToken(pathText, "<winAppData>", context.appdata, missing);
  pathText = replaceToken(pathText, "<winLocalAppData>", context.localAppData, missing);
  pathText = replaceToken(pathText, "<winLocalAppDataLow>", context.localLow, missing);
  pathText = replaceToken(pathText, "<winLocalLow>", context.localLow, missing);
  pathText = replaceToken(pathText, "<winSavedGames>", context.savedGames, missing);
  pathText = replaceToken(pathText, "<winPublic>", context.publicPath, missing);
  pathText = replaceToken(pathText, "<winPublicDocuments>", context.publicDocuments, missing);
  pathText = replaceToken(pathText, "<winProgramData>", context.programData, missing);
  pathText = replaceToken(pathText, "<steam>", context.steam, missing);
  pathText = replaceToken(pathText, "<steamUserData>", context.steamUserData, missing);
  pathText = replaceToken(pathText, "<steamuserdata>", context.steamUserData, missing);
  pathText = pathText.replaceAll("<storeUserId>", "*");
  if (missing.value) {
    return [];
  }
  pathText = expandEnvVars(pathText);
  pathText = expandTilde(pathText, context.home);
  if (pathText.includes("*") || pathText.includes("?")) {
    return expandGlobPattern(pathText);
  }
  return await pathExists(pathText) ? [pathText] : [];
}
async function manifestRoots(entry, context) {
  const roots = [];
  const filesMap = entry.files ?? {};
  for (const paths of Object.values(filesMap)) {
    for (const rawPath of paths) {
      const resolved = await resolvePath(rawPath, context);
      roots.push(...resolved);
    }
  }
  return roots;
}
async function findNamedPaths(base, names) {
  const out = [];
  for (const name of names) {
    const candidate = path13.join(base, name);
    if (await pathExists(candidate)) {
      out.push(candidate);
    }
  }
  return out;
}
async function findWindowsStorePaths(localAppData, gameName) {
  const packagesRoot = path13.join(localAppData, "Packages");
  if (!await pathExists(packagesRoot)) {
    return [];
  }
  const normalized = sanitizeName(gameName).replace(/\s+/g, "").toLowerCase();
  if (!normalized) {
    return [];
  }
  const matches = [];
  const entries = await readDirectoryEntries(packagesRoot);
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const name = entry.name.toLowerCase();
    if (!name.includes(normalized)) {
      continue;
    }
    const root = path13.join(packagesRoot, entry.name);
    for (const candidate of [
      path13.join(root, "SystemAppData", "wgs"),
      path13.join(root, "SystemAppData", "xgs"),
      path13.join(root, "LocalState")
    ]) {
      if (await pathExists(candidate)) {
        matches.push(candidate);
      }
    }
  }
  return matches;
}
async function readLibraryFolders(steamPath) {
  const libraryFile = path13.join(steamPath, "steamapps", "libraryfolders.vdf");
  if (!await pathExists(libraryFile)) {
    return [];
  }
  const text = await readFile5(libraryFile, "utf8");
  const out = [];
  for (const line of text.split(/\r?\n/)) {
    const parts = line.split('"');
    if (parts.length >= 4 && parts[1] === "path") {
      out.push(parts[3].replace(/\\\\/g, "\\"));
    }
  }
  return out;
}
async function findSteamLibraryPaths(steamPath) {
  const folders = [steamPath, ...await readLibraryFolders(steamPath)];
  const seen = new Set;
  const out = [];
  for (const folder of folders) {
    const normalized = WIN_PATH3.normalize(folder);
    if (!seen.has(normalized)) {
      seen.add(normalized);
      out.push(folder);
    }
  }
  return out;
}
function findAcfValue(text, key) {
  for (const line of text.split(/\r?\n/)) {
    const parts = line.split('"');
    if (parts.length >= 4 && parts[1] === key) {
      return parts[3];
    }
  }
  return null;
}
async function findSteamAppIds(gameName, libraryPaths) {
  const target = normalizeNameForMatch(gameName);
  const appIds = [];
  const seen = new Set;
  for (const library of libraryPaths) {
    const steamapps = path13.join(library, "steamapps");
    if (!await pathExists(steamapps)) {
      continue;
    }
    const entries = await readDirectoryEntries(steamapps);
    for (const entry of entries) {
      if (!entry.name.endsWith(".acf")) {
        continue;
      }
      const stem = entry.name.slice(0, -4);
      if (!stem.startsWith("appmanifest_")) {
        continue;
      }
      const appId = stem.slice("appmanifest_".length);
      if (seen.has(appId)) {
        continue;
      }
      const manifestPath = path13.join(steamapps, entry.name);
      const text = await readFile5(manifestPath, "utf8").catch(() => null);
      const name = text ? findAcfValue(text, "name") : null;
      if (!name) {
        continue;
      }
      const normalized = normalizeNameForMatch(name);
      if (similarityScore(target, normalized) >= 0.7) {
        seen.add(appId);
        appIds.push(appId);
      }
    }
  }
  return appIds;
}
async function findSteamSavePaths(gameName) {
  const steamPath = await findSteamPath();
  if (!steamPath) {
    return [];
  }
  const libraryPaths = await findSteamLibraryPaths(steamPath);
  const appIds = await findSteamAppIds(gameName, libraryPaths);
  if (appIds.length === 0) {
    return [];
  }
  const userdataRoot = path13.join(steamPath, "userdata");
  if (!await pathExists(userdataRoot)) {
    return [];
  }
  const out = [];
  const users = await readDirectoryEntries(userdataRoot);
  for (const user of users) {
    if (!user.isDirectory()) {
      continue;
    }
    const userPath = path13.join(userdataRoot, user.name);
    for (const appId of appIds) {
      const appRoot = path13.join(userPath, appId);
      if (!await pathExists(appRoot)) {
        continue;
      }
      for (const candidate of [path13.join(appRoot, "remote"), path13.join(appRoot, "local"), appRoot]) {
        if (await pathExists(candidate)) {
          out.push(candidate);
          break;
        }
      }
    }
  }
  return out;
}
async function heuristicRoots(gameName, context) {
  const variants = candidateNames(gameName);
  const roots = [];
  if (context.documents) {
    roots.push(...await findNamedPaths(path13.join(context.documents, "My Games"), variants));
    roots.push(...await findNamedPaths(path13.join(context.documents, "Saved Games"), variants));
    roots.push(...await findNamedPaths(context.documents, variants));
  }
  if (context.savedGames) {
    roots.push(...await findNamedPaths(context.savedGames, variants));
  }
  if (context.appdata) {
    roots.push(...await findNamedPaths(context.appdata, variants));
  }
  if (context.localAppData) {
    roots.push(...await findNamedPaths(context.localAppData, variants));
    roots.push(...await findWindowsStorePaths(context.localAppData, gameName));
  }
  if (context.localLow) {
    roots.push(...await findNamedPaths(context.localLow, variants));
  }
  roots.push(...await findSteamSavePaths(gameName));
  return roots;
}
function normalizeNameForMatch(name) {
  const cleaned = name.toLowerCase().replace(/[^a-z0-9]+/g, " ");
  const stopWords = new Set([
    "the",
    "a",
    "an",
    "edition",
    "definitive",
    "remastered",
    "goty",
    "game",
    "of",
    "year",
    "ultimate",
    "complete",
    "collection",
    "bundle",
    "deluxe",
    "enhanced",
    "hd"
  ]);
  return cleaned.split(/\s+/).map((token) => token.trim()).filter((token) => token.length > 0 && !stopWords.has(token)).join(" ");
}
async function buildRoots(paths) {
  const seen = new Set;
  const roots = [];
  for (const p of paths) {
    const normalized = WIN_PATH3.normalize(p).toLowerCase();
    if (seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    roots.push({
      label: `root-${roots.length}`,
      path: p
    });
  }
  return roots;
}
async function collectFiles(roots) {
  const files = [];
  const seen = new Set;
  let totalSize = 0;
  for (const root of roots) {
    if (await isFile(root.path)) {
      const size = (await statPath(root.path)).size;
      if (seen.has(WIN_PATH3.normalize(root.path))) {
        continue;
      }
      seen.add(WIN_PATH3.normalize(root.path));
      files.push({
        path: root.path,
        rootLabel: root.label,
        relativePath: path13.basename(root.path),
        size
      });
      totalSize += size;
      continue;
    }
    if (!await isDirectory(root.path)) {
      continue;
    }
    const stack = [root.path];
    while (stack.length > 0) {
      const currentDir = stack.pop();
      const entries = await readDirectoryEntries(currentDir);
      for (const entry of entries) {
        const childPath = path13.join(currentDir, entry.name);
        if (entry.isDirectory()) {
          stack.push(childPath);
          continue;
        }
        if (!entry.isFile()) {
          continue;
        }
        const normalized = WIN_PATH3.normalize(childPath);
        if (seen.has(normalized)) {
          continue;
        }
        seen.add(normalized);
        const metadata = await statPath(childPath);
        files.push({
          path: childPath,
          rootLabel: root.label,
          relativePath: path13.relative(root.path, childPath),
          size: metadata.size
        });
        totalSize += metadata.size;
      }
    }
  }
  return {
    roots,
    files,
    totalSize
  };
}
async function findGameSaveRoots(gameName, manifest, overridePath) {
  const context = await createContext();
  const roots = [];
  if (overridePath) {
    if (!await pathExists(overridePath)) {
      throw new Error(`Save path does not exist: ${overridePath}`);
    }
    roots.push(overridePath);
  }
  if (roots.length === 0) {
    const entry = findGameEntry(manifest, gameName);
    if (entry) {
      roots.push(...await manifestRoots(entry[1], context));
    }
  }
  if (roots.length === 0) {
    roots.push(...await heuristicRoots(gameName, context));
  }
  return buildRoots(roots);
}
async function findGameSaves(gameName, manifest, overridePath) {
  const roots = await findGameSaveRoots(gameName, manifest, overridePath);
  if (roots.length === 0) {
    return null;
  }
  const discovery = await collectFiles(roots);
  if (discovery.files.length === 0) {
    return null;
  }
  return discovery;
}
async function findSavePath(input) {
  const roots = await findGameSaveRoots(input.gameName, input.manifest, input.overridePath ?? null);
  return {
    savePath: roots.length === 1 ? roots[0].path : null,
    candidates: roots.map((root) => root.path)
  };
}
async function discoverBackupInfo(gameName, manifest, overridePath) {
  const discovery = await findGameSaves(gameName, manifest, overridePath);
  if (!discovery) {
    return null;
  }
  const firstRoot = discovery.roots[0]?.path ?? null;
  return {
    gameName,
    savePath: discovery.roots.length === 1 ? firstRoot : null,
    registryPath: null,
    totalSize: discovery.totalSize,
    files: discovery.files.map((file) => file.path)
  };
}
async function resolveSavePathTemplate(gameExePath, rawPath) {
  if (!rawPath.includes(SAVE_PATH_GAME_TOKEN)) {
    return rawPath;
  }
  const exeDir = path13.dirname(gameExePath);
  return rawPath.replaceAll(SAVE_PATH_GAME_TOKEN, exeDir);
}

// electron/main/services/backup/index.ts
function formatTimestamp(date) {
  const pad2 = (value) => String(value).padStart(2, "0");
  const pad3 = (value) => String(value).padStart(3, "0");
  return [
    `${pad2(date.getHours())}${pad2(date.getMinutes())}${pad2(date.getSeconds())}`,
    `${pad3(date.getMilliseconds())}`,
    `${pad2(date.getDate())}${pad2(date.getMonth() + 1)}${date.getFullYear()}`
  ].join("_");
}
async function emitProgress(listener, progress) {
  if (!listener) {
    return;
  }
  await listener(progress);
}
async function ensureBackupRoot(backupRoot) {
  await ensureDir(backupRoot);
}
async function createBackup(input) {
  const discovery = await findGameSaves(input.gameName, input.manifest ?? null, input.overridePath ?? null);
  if (!discovery || discovery.files.length === 0) {
    throw new Error(`No save data found for ${input.gameName}`);
  }
  await ensureBackupRoot(input.backupRoot);
  const gameFolder = input.gameYear ? `${sanitizeGameFolder(input.gameName)}-${input.gameYear}` : sanitizeGameFolder(input.gameName);
  const gameBackupDir = path14.join(input.backupRoot, gameFolder);
  await ensureDir(gameBackupDir);
  const timestamp = formatTimestamp(new Date);
  const useZip = input.mode === "zip" && !input.skipCompressionOnce;
  const backupPath = await buildUniqueBackupPath(gameBackupDir, timestamp, useZip);
  const createdAt = new Date().toISOString();
  await emitProgress(input.onProgress, {
    stage: "scan",
    current: "Scanning save files",
    done: 0,
    total: 0
  });
  let totalBytes = 0;
  let stagingDir = null;
  try {
    if (useZip) {
      stagingDir = await createTempDir("arrancador-backup-");
      const { totalBytes: stagedBytes } = await copyDiscoveryToDirectory(stagingDir, discovery, {
        onProgress: input.onProgress
      });
      totalBytes = stagedBytes;
      await compressBackupDirectory(backupPath, stagingDir, input.compressionLevel);
    } else {
      const result = await copyDiscoveryToDirectory(backupPath, discovery, {
        onProgress: input.onProgress
      });
      totalBytes = result.totalBytes;
    }
  } catch (error) {
    await removeBackupArtifact(backupPath);
    if (stagingDir) {
      await removeBackupArtifact(stagingDir);
    }
    throw error;
  }
  if (totalBytes === 0) {
    await removeBackupArtifact(backupPath);
    if (stagingDir) {
      await removeBackupArtifact(stagingDir);
    }
    throw new Error("No save data found for this game");
  }
  if (stagingDir) {
    await removeBackupArtifact(stagingDir);
  }
  const backup = {
    id: randomUUID3(),
    gameId: input.gameId,
    backupPath,
    backupSize: totalBytes,
    createdAt,
    isAuto: Boolean(input.isAuto),
    notes: input.notes ?? null
  };
  if (input.maxBackupsPerGame && input.maxBackupsPerGame > 0) {
    await pruneBackupsByLimit({
      backupRoot: input.backupRoot,
      gameName: input.gameName,
      gameYear: input.gameYear ?? null,
      maxBackups: input.maxBackupsPerGame
    });
  }
  await emitProgress(input.onProgress, {
    stage: "done",
    current: "Backup created",
    done: 0,
    total: 0
  });
  return backup;
}
async function restoreBackup(input) {
  await restoreBackupArtifact(input.backupPath, input.onProgress ?? null);
  await emitProgress(input.onProgress, {
    stage: "done",
    current: "Restore completed",
    done: 0,
    total: 0
  });
}
async function deleteBackup(input) {
  await removeBackupArtifact(input.backupPath);
}
async function checkBackupNeeded(input) {
  if (!input.currentSave) {
    return false;
  }
  if (!input.lastBackup) {
    return true;
  }
  const backupTime = Date.parse(input.lastBackup.createdAt);
  if (Number.isNaN(backupTime)) {
    return true;
  }
  for (const filePath of input.currentSave.files) {
    try {
      const fileStat = await stat6(filePath);
      if (fileStat.mtimeMs > backupTime) {
        return true;
      }
    } catch {}
  }
  return false;
}
async function checkRestoreNeeded(input) {
  if (!input.currentSave) {
    return {
      shouldRestore: false,
      backupId: null,
      currentSize: 0,
      backupSize: input.lastBackup?.backupSize ?? 0
    };
  }
  if (!input.lastBackup) {
    return {
      shouldRestore: false,
      backupId: null,
      currentSize: input.currentSave.totalSize,
      backupSize: 0
    };
  }
  return {
    shouldRestore: input.currentSave.totalSize < input.lastBackup.backupSize,
    backupId: input.lastBackup.id ?? null,
    currentSize: input.currentSave.totalSize,
    backupSize: input.lastBackup.backupSize
  };
}
function sanitizeGameFolder(name) {
  return name.replace(/[<>:"/\\|?*]/g, "").trim() || "game";
}

// electron/main/backend.ts
var runtimeState = null;
var ipcRegistered = false;
function getDbPath() {
  return path15.join(app2.getPath("userData"), "arrancador.db");
}
function getArkDbPath() {
  const override = process.env.ARK_DB_PATH?.trim();
  if (override) {
    return override;
  }
  return path15.join(app2.getPath("appData"), "Kepler", "ark.db");
}
async function getSettingsMap(db) {
  const rows = await queryAll(db, "SELECT key, value FROM settings");
  return Object.fromEntries(rows.map((row) => [row.key, row.value]));
}
async function getSetting(db, key) {
  const row = await queryOne(db, "SELECT value FROM settings WHERE key = ?1", [key]);
  return row?.value ?? null;
}
async function setSetting(db, key, value) {
  await execute(db, "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)", [key, value]);
}
async function getGameBackupState(db, id) {
  const row = await queryOne(db, `SELECT id, name, released, save_path, backup_enabled
     FROM games
     WHERE id = ?1`, [id]);
  if (!row) {
    throw new Error("Game not found");
  }
  return row;
}
function defaultBackupDirectory() {
  return path15.join(app2.getPath("userData"), "backups");
}
async function getBackupRoot(db) {
  const custom = (await getSetting(db, "backup_directory"))?.trim() ?? "";
  return custom || defaultBackupDirectory();
}
function mapBackupRow(row) {
  return {
    id: row.id,
    game_id: row.game_id,
    backup_path: row.backup_path,
    backup_size: Number(row.backup_size ?? 0),
    created_at: row.created_at,
    is_auto: Number(row.is_auto ?? 0) === 1,
    notes: row.notes ?? null
  };
}
function releasedYear(released) {
  const year = released?.split("-")[0]?.trim();
  return year ? year : null;
}
function emitRendererEvent(channel, payload) {
  for (const window of BrowserWindow.getAllWindows()) {
    window.webContents.send(channel, payload);
  }
}
async function loadManifestFromCache() {
  const cachePath = manifestCachePath(app2.getPath("userData"));
  return await loadGameManifestCache(cachePath);
}
async function resolveGameOverridePath(db, gameId, rawSavePath) {
  if (!rawSavePath?.trim()) {
    return null;
  }
  const row = await queryOne(db, "SELECT exe_path FROM games WHERE id = ?1", [gameId]);
  if (!row?.exe_path) {
    return rawSavePath;
  }
  return await resolveSavePathTemplate(row.exe_path, rawSavePath);
}
async function listGameBackups(db, gameId) {
  const rows = await queryAll(db, `SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
     FROM backups
     WHERE game_id = ?1
     ORDER BY created_at DESC`, [gameId]);
  return rows.map(mapBackupRow);
}
async function getLatestBackup(db, gameId) {
  const row = await queryOne(db, `SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
     FROM backups
     WHERE game_id = ?1
     ORDER BY created_at DESC
     LIMIT 1`, [gameId]);
  return row ? mapBackupRow(row) : null;
}
async function reconcileBackupRows(db, gameId, maxBackups) {
  const backups = await listGameBackups(db, gameId);
  const extra = backups.slice(maxBackups);
  for (const backup of extra) {
    await execute(db, "DELETE FROM backups WHERE id = ?1", [backup.id]);
    await deleteBackup({ backupPath: backup.backup_path }).catch(() => {
      return;
    });
  }
  const latest = backups[0] ?? null;
  await execute(db, `UPDATE games
     SET backup_count = ?1, last_backup = ?2
     WHERE id = ?3`, [Math.max(0, backups.length - extra.length), latest?.created_at ?? null, gameId]);
}
function createRuntimeServices(db) {
  const arkDbPath = getArkDbPath();
  const settingsRepository = {
    listSettings: () => getSettingsMap(db),
    upsertSettings: async (entries) => {
      await runInTransaction(db, async (tx) => {
        for (const [key, value] of Object.entries(entries)) {
          await execute(tx, "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)", [key, value]);
        }
      });
    },
    getSetting: (key) => getSetting(db, key),
    upsertSetting: (key, value) => setSetting(db, key, value),
    listScanDirectories: async () => {
      const rows = await queryAll(db, "SELECT path FROM scan_directories ORDER BY path ASC");
      return rows.map((row) => row.path);
    },
    addScanDirectory: async (dirPath) => {
      await execute(db, "INSERT OR IGNORE INTO scan_directories (path) VALUES (?1)", [dirPath]);
    },
    removeScanDirectory: async (dirPath) => {
      await execute(db, "DELETE FROM scan_directories WHERE path = ?1", [dirPath]);
    }
  };
  const usageReadModel = createGameUsageReadModel({
    legacyDb: db,
    arkDbPath
  });
  const playtimeRepository = createPlaytimeStatsRepository2(db, arkDbPath);
  const games = createGamesService({
    db,
    usageReadModel
  });
  const settings = createSettingsService(settingsRepository);
  const stats = createStatsService(playtimeRepository);
  const achievements = createAchievementsService({ db });
  const notifications = createNotificationsService({ db });
  const system = createSystemService();
  const metadata = createMetadataService({
    settings: {
      getRawgApiKey: () => getSetting(db, "rawg_api_key"),
      setRawgApiKey: async (key) => {
        await setSetting(db, "rawg_api_key", key);
      }
    },
    games: {
      applyRawgMetadata: async (update) => await games.updateGame({
        id: update.gameId,
        name: update.name,
        rawg_id: update.rawgId,
        description: update.description,
        released: update.released,
        background_image: update.background_image,
        metacritic: update.metacritic,
        rating: update.rating,
        genres: update.genres,
        platforms: update.platforms,
        developers: update.developers,
        publishers: update.publishers
      })
    }
  });
  const catalogue = createCatalogueService({
    db,
    library: {
      listGames: async () => {
        const items = await games.getAllGames();
        return items.map((item) => ({
          id: item.id,
          name: item.name,
          exe_name: item.exe_name
        }));
      }
    }
  });
  return {
    games,
    settings,
    stats,
    achievements,
    notifications,
    system,
    metadata,
    catalogue
  };
}
function getRuntimeState() {
  if (!runtimeState) {
    throw new Error("App runtime is not initialized");
  }
  return runtimeState;
}
function registerIpcHandlers() {
  const withRuntime = (handler) => {
    return async (_event, ...args) => await handler(getRuntimeState(), ...args);
  };
  ipcMain.handle("get_all_games", withRuntime(async ({ services }) => await services.games.getAllGames()));
  ipcMain.handle("get_game", withRuntime(async ({ services }, payload) => await services.games.getGame(payload.id)));
  ipcMain.handle("add_game", withRuntime(async ({ services }, payload) => await services.games.addGame(payload.game)));
  ipcMain.handle("add_games_batch", withRuntime(async ({ services }, payload) => await services.games.addGamesBatch(payload.games)));
  ipcMain.handle("update_game", withRuntime(async ({ services }, payload) => await services.games.updateGame(payload.update)));
  ipcMain.handle("delete_game", withRuntime(async ({ services }, payload) => {
    await services.games.deleteGame(payload.id);
  }));
  ipcMain.handle("toggle_favorite", withRuntime(async ({ services }, payload) => await services.games.toggleFavorite(payload.id)));
  ipcMain.handle("get_favorites", withRuntime(async ({ services }) => await services.games.getFavorites()));
  ipcMain.handle("record_game_launch", withRuntime(async ({ services }, payload) => await services.games.recordGameLaunch(payload.id)));
  ipcMain.handle("search_games", withRuntime(async ({ services }, payload) => await services.games.searchGames(payload.query)));
  ipcMain.handle("game_exists_by_path", withRuntime(async ({ services }, payload) => await services.games.gameExistsByPath(payload.exePath)));
  ipcMain.handle("is_game_installed", withRuntime(async ({ services }, payload) => await services.games.isGameInstalled(payload.id)));
  ipcMain.handle("launch_game", withRuntime(async ({ services }, payload) => {
    await services.games.launchGame(payload.id);
    await services.achievements.recordAchievementEvent("game_launch");
  }));
  ipcMain.handle("get_running_instances", withRuntime(async ({ services }, payload) => await services.games.getRunningInstances(payload.id)));
  ipcMain.handle("kill_game_processes", withRuntime(async ({ services }, payload) => await services.games.killGameProcesses(payload.id)));
  ipcMain.handle("resolve_shortcut_target", withRuntime(async ({ services }, payload) => await services.games.resolveShortcutTarget(payload.path)));
  ipcMain.handle("search_rawg", withRuntime(async ({ services }, payload) => await services.metadata.searchRawg(payload.query)));
  ipcMain.handle("get_rawg_game_details", withRuntime(async ({ services }, payload) => await services.metadata.getRawgGameDetails(payload.rawgId)));
  ipcMain.handle("apply_rawg_metadata", withRuntime(async ({ services }, payload) => await services.metadata.applyRawgMetadata(payload.gameId, payload.rawgId, payload.rename)));
  ipcMain.handle("set_rawg_api_key", withRuntime(async ({ services }, payload) => {
    await services.metadata.setRawgApiKey(payload.key);
  }));
  ipcMain.handle("get_rawg_api_key", withRuntime(async ({ services }) => await services.metadata.getRawgApiKey()));
  ipcMain.handle("get_all_achievements", withRuntime(async ({ services }, payload) => await services.achievements.getAllAchievements(payload.unlockedOnly)));
  ipcMain.handle("seed_default_achievements", withRuntime(async ({ services }) => await services.achievements.seedDefaultAchievements()));
  ipcMain.handle("record_achievement_event", withRuntime(async ({ services }, payload) => await services.achievements.recordAchievementEvent(payload.eventTrigger, payload.eventContext)));
  ipcMain.handle("get_all_settings", withRuntime(async ({ services }) => await services.settings.getAllSettings()));
  ipcMain.handle("update_settings", withRuntime(async ({ services }, payload) => {
    await services.settings.updateSettings(payload.settings);
  }));
  ipcMain.handle("get_setting", withRuntime(async ({ services }, payload) => await services.settings.getSetting(payload.key)));
  ipcMain.handle("set_setting", withRuntime(async ({ services }, payload) => {
    await services.settings.setSetting(payload.key, payload.value);
  }));
  ipcMain.handle("add_scan_directory", withRuntime(async ({ services }, payload) => {
    await services.settings.addScanDirectory(payload.path);
  }));
  ipcMain.handle("get_scan_directories", withRuntime(async ({ services }) => await services.settings.getScanDirectories()));
  ipcMain.handle("remove_scan_directory", withRuntime(async ({ services }, payload) => {
    await services.settings.removeScanDirectory(payload.path);
  }));
  ipcMain.handle("get_playtime_stats", withRuntime(async ({ services }, payload = {}) => await services.stats.getPlaytimeStats(payload?.start, payload?.end)));
  ipcMain.handle("get_running_processes", withRuntime(async () => await getRunningProcesses()));
  ipcMain.handle("get_system_info", withRuntime(async ({ services }) => await services.system.getSystemInfo()));
  ipcMain.handle("test_disk_speed", withRuntime(async ({ services }, payload) => await services.system.testDiskSpeed(payload.mountPoint)));
  ipcMain.handle("list_notifications", withRuntime(async ({ services }, payload) => await services.notifications.listNotifications(payload.unread_only)));
  ipcMain.handle("create_notification", withRuntime(async ({ services }, payload) => await services.notifications.createNotification(payload.level, payload.title, payload.message, payload.source)));
  ipcMain.handle("mark_notification_read", withRuntime(async ({ services }, payload) => await services.notifications.markNotificationRead(payload.id)));
  ipcMain.handle("mark_all_notifications_read", withRuntime(async ({ services }) => await services.notifications.markAllNotificationsRead()));
  ipcMain.handle("clear_notifications", withRuntime(async ({ services }) => await services.notifications.clearNotifications()));
  ipcMain.handle("get_catalogue_items", withRuntime(async ({ services }, payload) => await services.catalogue.getCatalogueItems(payload.source)));
  ipcMain.handle("search_catalogue", withRuntime(async ({ services }, payload) => await services.catalogue.searchCatalogue(payload.query, payload.source)));
  ipcMain.handle("upsert_catalogue_item", withRuntime(async ({ services }, payload) => await services.catalogue.upsertCatalogueItem({
    id: payload.input.id,
    rawgId: payload.input.rawg_id,
    name: payload.input.name,
    payload: payload.input.payload,
    source: payload.input.source
  })));
  ipcMain.handle("delete_catalogue_item", withRuntime(async ({ services }, payload) => await services.catalogue.deleteCatalogueItem(payload.id)));
  ipcMain.handle("sync_library_to_catalogue", withRuntime(async ({ services }) => await services.catalogue.syncLibraryToCatalogue()));
  ipcMain.handle("check_ludusavi_installed", withRuntime(async () => true));
  ipcMain.handle("get_ludusavi_executable_path", withRuntime(async ({ db }) => await getSetting(db, "ludusavi_path") || "native"));
  ipcMain.handle("set_ludusavi_path", withRuntime(async ({ db }, payload) => {
    await setSetting(db, "ludusavi_path", payload.path || "native");
  }));
  ipcMain.handle("set_backup_directory", withRuntime(async ({ db }, payload) => {
    await setSetting(db, "backup_directory", payload.path.trim());
  }));
  ipcMain.handle("get_backup_directory_setting", withRuntime(async ({ db }) => await getBackupRoot(db)));
  ipcMain.handle("refresh_sqoba_manifest", withRuntime(async () => {
    return;
  }));
  ipcMain.handle("find_game_save_paths", withRuntime(async ({ db }, payload) => {
    const manifest = await loadManifestFromCache();
    const gameState = payload.gameId ? await getGameBackupState(db, payload.gameId) : null;
    const overridePath = payload.gameId && gameState ? await resolveGameOverridePath(db, payload.gameId, gameState.save_path) : null;
    const result = await findSavePath({
      gameName: payload.gameName,
      gameId: payload.gameId ?? null,
      overridePath,
      manifest
    });
    if (!result.savePath && payload.gameId) {
      emitRendererEvent("game:save-path-missing", {
        game_id: payload.gameId,
        game_name: payload.gameName
      });
    }
    return {
      save_path: result.savePath,
      candidates: result.candidates
    };
  }));
  ipcMain.handle("find_game_saves", withRuntime(async ({ db }, payload) => {
    const manifest = await loadManifestFromCache();
    const gameState = payload.gameId ? await getGameBackupState(db, payload.gameId) : null;
    const overridePath = payload.gameId && gameState ? await resolveGameOverridePath(db, payload.gameId, gameState.save_path) : null;
    const result = await discoverBackupInfo(payload.gameName, manifest, overridePath);
    return result ? {
      game_name: result.gameName,
      save_path: result.savePath,
      registry_path: result.registryPath,
      total_size: result.totalSize,
      files: result.files
    } : null;
  }));
  ipcMain.handle("create_backup", withRuntime(async ({ db, services }, payload) => {
    const gameState = await getGameBackupState(db, payload.gameId);
    const manifest = await loadManifestFromCache();
    const overridePath = await resolveGameOverridePath(db, payload.gameId, gameState.save_path);
    const backupRoot = await getBackupRoot(db);
    const settings = await services.settings.getAllSettings();
    const backup = await createBackup({
      gameId: payload.gameId,
      gameName: payload.gameName,
      backupRoot,
      mode: settings.backup_compression_enabled && !settings.backup_skip_compression_once ? "zip" : "directory",
      compressionLevel: settings.backup_compression_level,
      skipCompressionOnce: settings.backup_skip_compression_once,
      overridePath,
      manifest,
      isAuto: payload.isAuto,
      notes: payload.notes,
      gameYear: releasedYear(gameState.released),
      maxBackupsPerGame: settings.max_backups_per_game,
      onProgress: async (progress) => {
        emitRendererEvent("backup:progress", {
          game_id: payload.gameId,
          stage: progress.stage,
          message: progress.current,
          done: progress.done,
          total: progress.total
        });
      }
    });
    await execute(db, `INSERT INTO backups (id, game_id, backup_path, backup_size, created_at, is_auto, notes)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)`, [
      backup.id ?? backup.backupPath,
      payload.gameId,
      backup.backupPath,
      backup.backupSize,
      backup.createdAt,
      payload.isAuto ? 1 : 0,
      backup.notes ?? null
    ]);
    await execute(db, `UPDATE games
       SET last_backup = ?1,
           backup_count = backup_count + 1,
           backup_enabled = 1
       WHERE id = ?2`, [backup.createdAt, payload.gameId]);
    await reconcileBackupRows(db, payload.gameId, settings.max_backups_per_game);
    if (settings.backup_skip_compression_once) {
      await setSetting(db, "backup_skip_compression_once", "false");
    }
    await services.achievements.recordAchievementEvent("backup_restore");
    return {
      id: backup.id ?? backup.backupPath,
      game_id: payload.gameId,
      backup_path: backup.backupPath,
      backup_size: backup.backupSize,
      created_at: backup.createdAt,
      is_auto: Boolean(backup.isAuto),
      notes: backup.notes ?? null
    };
  }));
  ipcMain.handle("get_game_backups", withRuntime(async ({ db }, payload) => await listGameBackups(db, payload.gameId)));
  ipcMain.handle("restore_backup", withRuntime(async ({ db, services }, payload) => {
    const backup = await queryOne(db, `SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
       FROM backups
       WHERE id = ?1`, [payload.backupId]);
    if (!backup) {
      throw new Error("Backup not found");
    }
    await restoreBackup({
      backupPath: backup.backup_path,
      onProgress: async (progress) => {
        emitRendererEvent("restore:progress", {
          game_id: backup.game_id,
          stage: progress.stage,
          message: progress.current,
          done: progress.done,
          total: progress.total
        });
      }
    });
    await services.achievements.recordAchievementEvent("backup_restore");
  }));
  ipcMain.handle("delete_backup", withRuntime(async ({ db }, payload) => {
    const backup = await queryOne(db, `SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
       FROM backups
       WHERE id = ?1`, [payload.backupId]);
    if (!backup) {
      return;
    }
    await deleteBackup({ backupPath: backup.backup_path });
    await execute(db, "DELETE FROM backups WHERE id = ?1", [payload.backupId]);
    await reconcileBackupRows(db, backup.game_id, Number.MAX_SAFE_INTEGER);
  }));
  ipcMain.handle("should_backup_before_launch", withRuntime(async ({ db }, payload) => {
    const game = await getGameBackupState(db, payload.gameId);
    const enabled = await getSetting(db, "backup_before_launch") === "true";
    return enabled && Number(game.backup_enabled ?? 0) === 1;
  }));
  ipcMain.handle("check_backup_needed", withRuntime(async ({ db }, payload) => {
    const game = await getGameBackupState(db, payload.gameId);
    const manifest = await loadManifestFromCache();
    const overridePath = await resolveGameOverridePath(db, payload.gameId, game.save_path);
    const currentSave = await findGameSaves(payload.gameName, manifest, overridePath);
    const lastBackup = await getLatestBackup(db, payload.gameId);
    return await checkBackupNeeded({
      currentSave,
      lastBackup: lastBackup ? {
        id: lastBackup.id,
        gameId: lastBackup.game_id,
        backupPath: lastBackup.backup_path,
        backupSize: lastBackup.backup_size,
        createdAt: lastBackup.created_at,
        isAuto: lastBackup.is_auto,
        notes: lastBackup.notes
      } : null
    });
  }));
  ipcMain.handle("check_restore_needed", withRuntime(async ({ db }, payload) => {
    const game = await getGameBackupState(db, payload.gameId);
    const manifest = await loadManifestFromCache();
    const overridePath = await resolveGameOverridePath(db, payload.gameId, game.save_path);
    const currentSave = await findGameSaves(payload.gameName, manifest, overridePath);
    const lastBackup = await getLatestBackup(db, payload.gameId);
    const result = await checkRestoreNeeded({
      currentSave,
      lastBackup: lastBackup ? {
        id: lastBackup.id,
        gameId: lastBackup.game_id,
        backupPath: lastBackup.backup_path,
        backupSize: lastBackup.backup_size,
        createdAt: lastBackup.created_at,
        isAuto: lastBackup.is_auto,
        notes: lastBackup.notes
      } : null
    });
    return {
      should_restore: result.shouldRestore,
      backup_id: result.backupId,
      current_size: result.currentSize,
      backup_size: result.backupSize
    };
  }));
  ipcMain.handle("get_backup_settings", withRuntime(async ({ db }) => {
    const rows = await queryAll(db, `SELECT key, value
       FROM settings
       WHERE key LIKE 'backup%' OR key = 'ludusavi_path' OR key = 'max_backups_per_game'
       ORDER BY key ASC`);
    return Object.fromEntries(rows.map((row) => [row.key, row.value]));
  }));
  ipcMain.handle("update_backup_settings", withRuntime(async ({ db }, payload) => {
    await runInTransaction(db, async (tx) => {
      for (const [key, value] of Object.entries(payload.settings)) {
        await execute(tx, "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)", [key, String(value)]);
      }
    });
  }));
  ipcMain.handle("dialog_open", withRuntime(async (_runtime, payload) => {
    const window = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    const result = window ? await dialog.showOpenDialog(window, payload) : await dialog.showOpenDialog(payload);
    if (result.canceled) {
      return null;
    }
    if (payload.multiple) {
      return result.filePaths;
    }
    return result.filePaths[0] ?? null;
  }));
  ipcMain.handle("shell_open_path", withRuntime(async (_runtime, payload) => await shell.openPath(payload.path)));
  ipcMain.handle("shell_open_external", withRuntime(async (_runtime, payload) => {
    await shell.openExternal(payload.url);
  }));
  ipcMain.handle("get_autostart_state", withRuntime(async () => app2.getLoginItemSettings().openAtLogin));
  ipcMain.handle("set_autostart_state", withRuntime(async (_runtime, payload) => {
    app2.setLoginItemSettings({
      openAtLogin: payload.enabled,
      path: process.execPath
    });
  }));
  ipcMain.handle("cancel_scan", withRuntime(async (runtime) => {
    runtime.currentScan?.cancel();
    runtime.currentScan = null;
  }));
  ipcMain.handle("scan_executables_stream", withRuntime(async (runtime, payload) => {
    runtime.currentScan?.cancel();
    const cancellation = createScanCancellation();
    runtime.currentScan = cancellation;
    let count = 0;
    try {
      count = await scanExecutablesStream(payload.dir, {
        signal: cancellation.signal,
        onEntry: async (entry) => {
          emitRendererEvent("scan:entry", entry);
        }
      });
      await runtime.services.achievements.recordAchievementEvent("scan_complete");
      return count;
    } finally {
      if (runtime.currentScan === cancellation) {
        runtime.currentScan = null;
      }
      emitRendererEvent("scan:done", { count });
    }
  }));
}
async function createRuntime() {
  const db = await openGameDatabase(openSqliteDatabase(getDbPath()));
  runtimeState = {
    db,
    services: createRuntimeServices(db),
    currentScan: null
  };
}
async function initializeBackend() {
  if (!runtimeState) {
    await createRuntime();
  }
  if (!ipcRegistered) {
    registerIpcHandlers();
    ipcRegistered = true;
  }
}

// electron/main/tray.ts
import { Menu, Tray } from "electron";
function createAppTray(options) {
  const tray = new Tray(options.icon);
  tray.setToolTip("Arrancador");
  const menu = Menu.buildFromTemplate([
    {
      label: "Показать или скрыть окно",
      click: () => options.onToggle()
    },
    {
      type: "separator"
    },
    {
      label: "Показать окно",
      click: () => options.onShow()
    },
    {
      label: "Скрыть в трей",
      click: () => options.onHide()
    },
    {
      type: "separator"
    },
    {
      label: "Выход",
      click: () => {
        options.onQuit();
      }
    }
  ]);
  tray.setContextMenu(menu);
  tray.on("click", () => options.onToggle());
  return tray;
}

// electron/main/windows.ts
import fs3 from "node:fs";
import path16 from "node:path";
import { BrowserWindow as BrowserWindow2, nativeImage, shell as shell2 } from "electron";
function createFallbackIcon() {
  const svg = `
<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#1f2937"/>
      <stop offset="100%" stop-color="#0f172a"/>
    </linearGradient>
    <linearGradient id="accent" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#f59e0b"/>
      <stop offset="100%" stop-color="#f97316"/>
    </linearGradient>
  </defs>
  <rect x="8" y="8" width="112" height="112" rx="28" fill="url(#bg)"/>
  <rect x="30" y="34" width="68" height="12" rx="6" fill="url(#accent)"/>
  <rect x="30" y="56" width="52" height="12" rx="6" fill="#cbd5e1"/>
  <rect x="30" y="78" width="38" height="12" rx="6" fill="#94a3b8"/>
</svg>`.trim();
  return nativeImage.createFromDataURL(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`);
}
function tryCreateIconFromFile(filePath) {
  if (!fs3.existsSync(filePath)) {
    return null;
  }
  const image = nativeImage.createFromPath(filePath);
  return image.isEmpty() ? null : image;
}
async function resolveWindowIcon(options) {
  const candidates = [
    path16.join(options.resourcesPath, "icon.png"),
    path16.join(options.resourcesPath, "32x32.png")
  ];
  for (const candidate of candidates) {
    const image = tryCreateIconFromFile(candidate);
    if (image) {
      return image;
    }
  }
  return createFallbackIcon();
}
function createMainWindow(options) {
  const isDev = Boolean(options.rendererUrl);
  const window = new BrowserWindow2({
    width: options.width,
    height: options.height,
    minWidth: options.minWidth,
    minHeight: options.minHeight,
    title: options.title,
    show: isDev,
    autoHideMenuBar: true,
    backgroundColor: "#0f131a",
    icon: options.icon,
    webPreferences: {
      preload: options.preloadPath,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  });
  window.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:\/\//i.test(url)) {
      shell2.openExternal(url);
    }
    return { action: "deny" };
  });
  window.once("ready-to-show", () => {
    if (!window.isDestroyed()) {
      window.show();
      window.focus();
    }
  });
  window.webContents.on("preload-error", (_event, preloadPath, error) => {
    console.error("Preload failed:", JSON.stringify({
      preloadPath,
      message: error.message,
      stack: error.stack
    }));
  });
  window.webContents.on("did-fail-load", (_event, errorCode, errorDescription, validatedURL) => {
    console.error("Renderer failed to load:", JSON.stringify({ errorCode, errorDescription, validatedURL }));
    if (!window.isDestroyed() && !window.isVisible()) {
      window.show();
    }
  });
  window.webContents.on("did-finish-load", () => {
    if (!window.isDestroyed() && !window.isVisible()) {
      window.show();
      window.focus();
    }
  });
  window.webContents.on("render-process-gone", (_event, details) => {
    console.error("Renderer process exited:", details);
  });
  const showFallbackTimer = setTimeout(() => {
    if (!window.isDestroyed() && !window.isVisible()) {
      window.show();
      window.focus();
    }
  }, 2000);
  window.once("closed", () => {
    clearTimeout(showFallbackTimer);
  });
  if (options.rendererUrl) {
    window.loadURL(options.rendererUrl);
  } else {
    window.loadFile(path16.join(options.appPath, "out", "renderer", "index.html"));
  }
  return window;
}
function showMainWindow(window) {
  if (window.isDestroyed()) {
    return;
  }
  if (window.isMinimized()) {
    window.restore();
  }
  window.show();
  window.focus();
}
function hideMainWindow(window) {
  if (!window.isDestroyed()) {
    window.hide();
  }
}

// electron/main/helpers/user-data.ts
import os5 from "node:os";
import path17 from "node:path";
var DEFAULT_APP_NAME = "arrancador";
function resolveSharedDataRoot() {
  if (process.platform === "win32") {
    return process.env.LOCALAPPDATA ?? path17.join(os5.homedir(), "AppData", "Local");
  }
  if (process.platform === "darwin") {
    return path17.join(os5.homedir(), "Library", "Application Support");
  }
  return process.env.XDG_DATA_HOME ?? path17.join(os5.homedir(), ".local", "share");
}
function resolveSharedUserDataPath(appName = DEFAULT_APP_NAME) {
  return path17.join(resolveSharedDataRoot(), appName);
}

// electron/main/index.ts
var APP_NAME = "arrancador";
var DISPLAY_NAME = "Arrancador";
var APP_ID = "com.arrancador.app";
var ROOT_DIR = fileURLToPath(new URL(".", import.meta.url));
var mainWindow = null;
var tray = null;
var quitting = false;
var windowCreationPromise = null;
function getRendererUrl() {
  const url = process.env.ELECTRON_RENDERER_URL ?? process.env.VITE_DEV_SERVER_URL ?? process.env.ELECTRON_VITE_DEV_SERVER_URL ?? null;
  return url && url.trim().length > 0 ? url : null;
}
function getPreloadPath() {
  return path18.join(ROOT_DIR, "../preload/index.js");
}
async function startWindow() {
  if (mainWindow && !mainWindow.isDestroyed()) {
    return mainWindow;
  }
  if (windowCreationPromise) {
    return windowCreationPromise;
  }
  windowCreationPromise = (async () => {
    mainWindow = null;
    const icon = await resolveWindowIcon({
      resourcesPath: process.resourcesPath
    });
    const window = createMainWindow({
      appPath: app3.getAppPath(),
      preloadPath: getPreloadPath(),
      rendererUrl: getRendererUrl(),
      icon,
      title: DISPLAY_NAME,
      width: 1200,
      height: 800,
      minWidth: 1200,
      minHeight: 800
    });
    window.on("close", (event) => {
      if (quitting) {
        return;
      }
      event.preventDefault();
      hideMainWindow(window);
    });
    window.on("minimize", (event) => {
      if (quitting) {
        return;
      }
      event.preventDefault();
      hideMainWindow(window);
    });
    mainWindow = window;
    return window;
  })();
  try {
    return await windowCreationPromise;
  } finally {
    windowCreationPromise = null;
  }
}
async function shutdownApp() {
  if (quitting) {
    return;
  }
  quitting = true;
  tray?.destroy();
  tray = null;
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.removeAllListeners("close");
    mainWindow.removeAllListeners("minimize");
    mainWindow.destroy();
  }
  mainWindow = null;
  app3.exit(0);
}
async function runApp() {
  app3.setAppUserModelId(APP_ID);
  app3.setName(DISPLAY_NAME);
  app3.setPath("userData", resolveSharedUserDataPath(APP_NAME));
  if (!app3.requestSingleInstanceLock()) {
    app3.quit();
    return;
  }
  app3.on("second-instance", () => {
    if (mainWindow && !mainWindow.isDestroyed()) {
      showMainWindow(mainWindow);
      return;
    }
    mainWindow = null;
    startWindow().then((window) => {
      showMainWindow(window);
    });
  });
  app3.on("activate", () => {
    if (mainWindow && !mainWindow.isDestroyed()) {
      showMainWindow(mainWindow);
      return;
    }
    mainWindow = null;
    startWindow();
  });
  app3.on("before-quit", (event) => {
    if (quitting) {
      return;
    }
    event.preventDefault();
    shutdownApp();
  });
  await app3.whenReady();
  await mkdir2(resolveSharedUserDataPath(APP_NAME), { recursive: true });
  await initializeBackend();
  await startWindow();
  tray = createAppTray({
    icon: await resolveWindowIcon({
      resourcesPath: process.resourcesPath
    }),
    onToggle: () => {
      if (!mainWindow || mainWindow.isDestroyed()) {
        mainWindow = null;
        startWindow();
        return;
      }
      if (mainWindow.isVisible()) {
        hideMainWindow(mainWindow);
      } else {
        showMainWindow(mainWindow);
      }
    },
    onShow: () => {
      if (!mainWindow || mainWindow.isDestroyed()) {
        mainWindow = null;
        startWindow().then((window) => {
          showMainWindow(window);
        });
        return;
      }
      showMainWindow(mainWindow);
    },
    onHide: () => {
      if (mainWindow && !mainWindow.isDestroyed()) {
        hideMainWindow(mainWindow);
      }
    },
    onQuit: () => {
      shutdownApp();
    }
  });
}

// electron/main.ts
runApp().catch((error) => {
  console.error("Electron app failed to start:", error);
  process.exitCode = 1;
});
