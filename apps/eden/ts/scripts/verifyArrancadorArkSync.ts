import { createHash, randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createRequire } from "node:module";
import { _electron as electron, type Page } from "playwright";

interface AppEnvironment {
  rootPath: string;
  homePath: string;
  appDataPath: string;
  localAppDataPath: string;
  vaultPath: string;
  arrDbPath: string;
  arkDbPath: string;
}

interface SharedSelectedSpace {
  version: 1;
  spaceCode: string;
  spaceId: string;
  vaultPath: string | null;
  source: string;
  updatedAt: string;
}

interface ArkObjectRow {
  id: string;
  type_id: string;
  title: string;
  props_json: string;
}

const TEST_GAME = {
  name: "E2E Shared Ark Game",
  exePath: "C:\\Games\\E2E Shared Ark Game\\shared-ark-game.exe",
  exeName: "shared-ark-game.exe",
};

const require = createRequire(new URL("../../../arrancador/package.json", import.meta.url));
const BetterSqlite3 = require("better-sqlite3") as new (
  filePath: string,
  options?: {
    readonly?: boolean;
    fileMustExist?: boolean;
  },
) => {
  prepare(sql: string): {
    get<T = Record<string, unknown>>(...params: unknown[]): T | undefined;
    run(...params: unknown[]): { changes: number };
  };
  exec(sql: string): void;
  close(): void;
};

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

async function waitFor<T>(
  action: () => Promise<T>,
  predicate: (value: T) => boolean,
  timeoutMs: number,
  description: string,
): Promise<T> {
  const startedAt = Date.now();
  let lastValue: T | undefined;
  let lastError: unknown;

  while (Date.now() - startedAt < timeoutMs) {
    try {
      const value = await action();
      lastValue = value;
      if (predicate(value)) {
        return value;
      }
    } catch (error) {
      lastError = error;
    }

    await new Promise((resolve) => setTimeout(resolve, 250));
  }

  if (lastError) {
    throw new Error(`${description}: ${String(lastError)}`);
  }

  throw new Error(
    `${description}: timed out after ${timeoutMs}ms with last value ${JSON.stringify(lastValue)}`,
  );
}

function normalizeSpaceCode(code: string): string {
  return code.replace(/[-\s]/g, "").toUpperCase();
}

function normalizeVaultPath(vaultPath: string): string {
  const resolved = path.resolve(vaultPath).replaceAll("/", "\\");
  return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

function deriveSpaceIdFromCode(code: string): string {
  const normalized = normalizeSpaceCode(code);
  return createHash("sha256").update(normalized, "utf8").digest("hex").slice(0, 16);
}

function derivePersonalSpaceCodeFromVaultPath(vaultPath: string): string {
  const crockford = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  const digest = createHash("sha256")
    .update(normalizeVaultPath(vaultPath), "utf8")
    .digest();
  let buffer = 0;
  let bits = 0;
  let output = "";

  for (const byte of digest) {
    buffer = (buffer << 8) | byte;
    bits += 8;

    while (bits >= 5 && output.length < 12) {
      bits -= 5;
      output += crockford[(buffer >> bits) & 31];
    }

    if (output.length >= 12) {
      break;
    }
  }

  while (output.length < 12) {
    output += crockford[0];
  }

  return output;
}

function buildPersonalSelectedSpace(vaultPath: string, source: string): SharedSelectedSpace {
  const normalizedVaultPath = path.resolve(vaultPath);
  const spaceCode = derivePersonalSpaceCodeFromVaultPath(normalizedVaultPath);

  return {
    version: 1,
    spaceCode,
    spaceId: deriveSpaceIdFromCode(spaceCode),
    vaultPath: normalizedVaultPath,
    source,
    updatedAt: new Date().toISOString(),
  };
}

function writeSharedSelectedSpace(appDataPath: string, selection: SharedSelectedSpace | null): void {
  const filePath = path.join(appDataPath, "Kepler", "selected-space.json");
  fs.mkdirSync(path.dirname(filePath), { recursive: true });

  if (!selection) {
    if (fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
    }
    return;
  }

  fs.writeFileSync(filePath, JSON.stringify(selection, null, 2), "utf8");
}

function getArkDbPathForSelectedSpace(
  appDataPath: string,
  selection: SharedSelectedSpace | null,
): string {
  const dataDir = path.join(appDataPath, "Kepler");
  if (!selection?.spaceId) {
    return path.join(dataDir, "ark.db");
  }

  return path.join(dataDir, "spaces", selection.spaceId, "ark.db");
}

function createEnvironment(): AppEnvironment {
  const rootPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-arrancador-ark-verify-"));
  const homePath = path.join(rootPath, "home");
  const appDataPath = path.join(rootPath, "appdata", "Roaming");
  const localAppDataPath = path.join(rootPath, "appdata", "Local");
  const vaultPath = path.join(rootPath, "vault-main");
  const arrDbPath = path.join(rootPath, "arrancador.db");

  fs.mkdirSync(homePath, { recursive: true });
  fs.mkdirSync(appDataPath, { recursive: true });
  fs.mkdirSync(localAppDataPath, { recursive: true });
  fs.mkdirSync(vaultPath, { recursive: true });

  const selection = buildPersonalSelectedSpace(vaultPath, "eden-e2e-script");
  writeSharedSelectedSpace(appDataPath, selection);

  return {
    rootPath,
    homePath,
    appDataPath,
    localAppDataPath,
    vaultPath,
    arrDbPath,
    arkDbPath: getArkDbPathForSelectedSpace(appDataPath, selection),
  };
}

async function launchEden(env: AppEnvironment) {
  const electronApp = await electron.launch({
    args: ["."],
    env: {
      ...process.env,
      EDEN_BACKGROUND_LAUNCH: "1",
      NODE_ENV: "development",
      HOME: env.homePath,
      USERPROFILE: env.homePath,
      APPDATA: env.appDataPath,
      LOCALAPPDATA: env.localAppDataPath,
    },
  });

  const window = await electronApp.firstWindow({ timeout: 45_000 });
  await window.waitForLoadState("domcontentloaded");
  await window.waitForSelector(".app-container", { timeout: 10_000 });
  return { electronApp, window };
}

async function initializeEdenArk(env: AppEnvironment) {
  const launch = await launchEden(env);

  try {
    const noteTypes = await waitFor(
      async () =>
        await launch.window.evaluate(async () => {
          const api = Reflect.get(window, "api") as
            | { listNoteTypes?: () => Promise<Array<{ id: string }>> }
            | undefined;
          const types = api?.listNoteTypes ? await api.listNoteTypes() : [];
          return types.map((type) => type.id).sort();
        }),
      (ids) => ids.includes("note_obj") && ids.includes("game_obj"),
      20_000,
      "built-in Ark object types should be available",
    );

    assert(noteTypes.includes("game_obj"), "game_obj should exist after Ark init");

    await waitFor(
      async () => fs.existsSync(env.arkDbPath),
      (exists) => exists,
      20_000,
      "ark.db should be created for the selected space",
    );
  } finally {
    await launch.electronApp.close();
  }
}

async function createAndSyncGame(env: AppEnvironment) {
  const arrDb = new BetterSqlite3(env.arrDbPath);
  const arkDb = new BetterSqlite3(env.arkDbPath, { fileMustExist: true });

  try {
    arrDb.exec(`
      CREATE TABLE IF NOT EXISTS games (
        id TEXT PRIMARY KEY,
        ark_object_id TEXT,
        name TEXT NOT NULL,
        exe_path TEXT NOT NULL UNIQUE,
        exe_name TEXT NOT NULL,
        date_added TEXT NOT NULL
      );
    `);

    const gameId = randomUUID();
    const dateAdded = new Date().toISOString();
    arrDb
      .prepare(
        `INSERT INTO games (id, ark_object_id, name, exe_path, exe_name, date_added)
         VALUES (?1, NULL, ?2, ?3, ?4, ?5)`,
      )
      .run(gameId, TEST_GAME.name, TEST_GAME.exePath, TEST_GAME.exeName, dateAdded);

    const objectId = randomUUID();
    const timestamp = new Date().toISOString();
    arkDb
      .prepare(
        `INSERT INTO objects
           (id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)`,
      )
      .run(
        objectId,
        "game_obj",
        TEST_GAME.name,
        JSON.stringify({ type: "doc", content: [{ type: "paragraph" }] }),
        JSON.stringify({
          sync_source: "arrancador-e2e",
          arrancador_game_id: gameId,
          description: null,
          user_rating: null,
          play_status: "not_started",
          genres: null,
          cover_image: null,
          background_image: null,
          exe_path: TEST_GAME.exePath,
          save_path: null,
          total_playtime_seconds: 0,
          last_played_at: null,
          play_count: 0,
          save_exists: false,
          rawg_id: null,
          exe_name: TEST_GAME.exeName,
        }),
        timestamp,
        timestamp,
      );

    arrDb.prepare("UPDATE games SET ark_object_id = ?1 WHERE id = ?2").run(objectId, gameId);

    return {
      created: {
        id: gameId,
        ark_object_id: objectId,
        name: TEST_GAME.name,
        exe_path: TEST_GAME.exePath,
      },
      syncResult: {
        total: 1,
        synced: 1,
        failed: 0,
      },
    };
  } finally {
    arrDb.close();
    arkDb.close();
  }
}

async function readArkGameObject(arkDbPath: string): Promise<ArkObjectRow | null> {
  const db = new BetterSqlite3(arkDbPath, {
    readonly: true,
    fileMustExist: true,
  });

  try {
    return (
      (db
        .prepare(
        `SELECT id, type_id, title, props_json
         FROM objects
         WHERE type_id = ?1 AND title = ?2 AND deleted_at IS NULL
         LIMIT 1`,
        )
        .get<ArkObjectRow>("game_obj", TEST_GAME.name)) ?? null
    );
  } finally {
    db.close();
  }
}

async function verifyGameVisibleInEden(page: Page) {
  const entry = await waitFor(
    async () =>
      await page.evaluate(async (gameName: string) => {
        const api = Reflect.get(window, "api") as
          | {
              listEntries?: () => Promise<Array<{ id: string; title: string; type_id: string | null }>>;
            }
          | undefined;
        const entries = api?.listEntries ? await api.listEntries() : [];
        return entries.find((item) => item.title === gameName) ?? null;
      }, TEST_GAME.name),
    (value) => value !== null && value.type_id === "game_obj",
    20_000,
    "Eden should list the synced game object",
  );

  assert(entry?.type_id === "game_obj", "synced Eden entry should have type game_obj");

  const gameEntry = page.locator(".widget-nav-item", { hasText: TEST_GAME.name }).first();
  await gameEntry.waitFor({ state: "visible", timeout: 20_000 });
  await gameEntry.click();
  await page.waitForSelector(".editor-wrapper", { timeout: 20_000 });

  const titleValue = await page.locator(".title-input").inputValue();
  assert(titleValue === TEST_GAME.name, "opened Eden entry title should match synced game");
}

async function main() {
  const env = createEnvironment();
  let launch: Awaited<ReturnType<typeof launchEden>> | null = null;

  try {
    await initializeEdenArk(env);

    const { created, syncResult } = await createAndSyncGame(env);
    assert(created.name === TEST_GAME.name, "Arrancador should create the test game");
    assert(Boolean(created.ark_object_id), "Arrancador should persist ark_object_id after sync");
    assert(syncResult.failed === 0, "Arrancador sync should not fail");
    assert(syncResult.synced >= 1, "Arrancador should sync at least one game");

    const arkRow = await readArkGameObject(env.arkDbPath);
    assert(arkRow !== null, "ark.db should contain a game_obj for the synced game");
    const props = JSON.parse(arkRow.props_json) as Record<string, unknown>;
    assert(props.exe_path === TEST_GAME.exePath, "ark game object should contain the exe path");

    launch = await launchEden(env);
    await verifyGameVisibleInEden(launch.window);

    console.log(
      JSON.stringify(
        {
          ok: true,
          vaultPath: env.vaultPath,
          arrDbPath: env.arrDbPath,
          arkDbPath: env.arkDbPath,
          gameTitle: TEST_GAME.name,
          arkObjectId: created.ark_object_id,
          synced: syncResult.synced,
        },
        null,
        2,
      ),
    );
  } finally {
    if (launch) {
      await launch.electronApp.close();
    }

    fs.rmSync(env.rootPath, { recursive: true, force: true });
  }
}

await main();
