import { test, expect, _electron as electron, type Page } from "@playwright/test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { createRequire as createNodeRequire } from "node:module";
import { openSqliteDatabase } from "../../../arrancador/electron/main/db/sqlite.ts";
import { openGameDatabase } from "../../../arrancador/electron/main/db/database.ts";
import { createArkGameObjectService } from "../../../arrancador/electron/main/services/ark-game-objects.ts";
import { createGamesService } from "../../../arrancador/electron/main/services/games.ts";

type ElectronApp = Awaited<ReturnType<typeof electron.launch>>;

interface LaunchedApp {
  electronApp: ElectronApp;
  window: Page;
  pageErrors: string[];
}

interface AppEnvironment {
  rootPath: string;
  homePath: string;
  appDataPath: string;
  localAppDataPath: string;
  vaultPath: string;
  arrDbPath: string;
  arkDbPath: string;
}

interface ArkObjectRow {
  id: string;
  type_id: string;
  title: string;
  props_json: string;
}

interface SharedSelectedSpace {
  version: 1;
  spaceCode: string;
  spaceId: string;
  vaultPath: string | null;
  source: string;
  updatedAt: string;
}

const TEST_GAME = {
  name: "E2E Shared Ark Game",
  exePath: "C:\\Games\\E2E Shared Ark Game\\shared-ark-game.exe",
  exeName: "shared-ark-game.exe",
};

const require = createNodeRequire(new URL("../../../arrancador/package.json", import.meta.url));
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
  const rootPath = fs.mkdtempSync(path.join(os.tmpdir(), "eden-arrancador-ark-"));
  const homePath = path.join(rootPath, "home");
  const appDataPath = path.join(rootPath, "appdata", "Roaming");
  const localAppDataPath = path.join(rootPath, "appdata", "Local");
  const vaultPath = path.join(rootPath, "vault-main");
  const arrDbPath = path.join(rootPath, "arrancador.db");

  fs.mkdirSync(homePath, { recursive: true });
  fs.mkdirSync(appDataPath, { recursive: true });
  fs.mkdirSync(localAppDataPath, { recursive: true });
  fs.mkdirSync(vaultPath, { recursive: true });

  const selection = buildPersonalSelectedSpace(vaultPath, "eden-e2e");
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

async function launchEden(env: AppEnvironment, attempt = 0): Promise<LaunchedApp> {
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

  try {
    const window = await electronApp.firstWindow({ timeout: 45_000 });
    const pageErrors: string[] = [];

    window.on("pageerror", (error) => {
      pageErrors.push(error.message);
    });

    await window.waitForLoadState("domcontentloaded");
    await window.waitForSelector(".app-container", { timeout: 10_000 });

    return {
      electronApp,
      window,
      pageErrors,
    };
  } catch (error) {
    await electronApp.close();

    if (attempt >= 2) {
      throw error;
    }

    return await launchEden(env, attempt + 1);
  }
}

async function initializeEdenArk(env: AppEnvironment) {
  const launch = await launchEden(env);

  try {
    await expect
      .poll(
        async () =>
          await launch.window.evaluate(async () => {
            const api = Reflect.get(window, "api") as
              | { listNoteTypes?: () => Promise<Array<{ id: string }>> }
              | undefined;
            const noteTypes = api?.listNoteTypes ? await api.listNoteTypes() : [];
            return noteTypes.map((type) => type.id).sort();
          }),
        { timeout: 20_000 },
      )
      .toEqual(expect.arrayContaining(["game_obj", "note_obj"]));

    await expect
      .poll(() => fs.existsSync(env.arkDbPath), { timeout: 20_000 })
      .toBe(true);
  } finally {
    await launch.electronApp.close();
  }
}

async function createAndSyncGame(env: AppEnvironment) {
  const arrDb = openSqliteDatabase(env.arrDbPath);
  await openGameDatabase(arrDb);

  try {
    const gamesService = createGamesService({
      db: arrDb,
      arkGameObjectSync: createArkGameObjectService({
        arkDbPath: env.arkDbPath,
      }),
    });

    const created = await gamesService.addGame({
      name: TEST_GAME.name,
      exe_path: TEST_GAME.exePath,
      exe_name: TEST_GAME.exeName,
    });

    return {
      created,
      syncResult: {
        total: 1,
        synced: 1,
        failed: 0,
      },
    };
  } finally {
    arrDb.close?.();
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

test.describe("Arrancador Ark sync visible in Eden", () => {
  test.describe.configure({ mode: "serial" });

  test("creates an isolated game, syncs it into Ark, shows it in Eden, and cleans up temp DBs", async () => {
    test.setTimeout(120_000);

    const env = createEnvironment();
    let launch: LaunchedApp | null = null;

    try {
      await initializeEdenArk(env);

      const { created, syncResult } = await createAndSyncGame(env);

      expect(syncResult.total).toBeGreaterThanOrEqual(1);
      expect(syncResult.failed).toBe(0);
      expect(syncResult.synced).toBeGreaterThanOrEqual(1);
      expect(created.name).toBe(TEST_GAME.name);
      expect(created.ark_object_id).toBeTruthy();

      expect(fs.existsSync(env.arrDbPath)).toBe(true);
      expect(fs.existsSync(env.arkDbPath)).toBe(true);

      const arkRow = await readArkGameObject(env.arkDbPath);
      expect(arkRow).not.toBeNull();
      expect(arkRow?.type_id).toBe("game_obj");
      expect(arkRow?.title).toBe(TEST_GAME.name);
      const arkProps = JSON.parse(arkRow?.props_json ?? "{}") as Record<string, unknown>;
      expect(arkProps.exe_path).toBe(TEST_GAME.exePath);

      launch = await launchEden(env);

      await expect
        .poll(
          async () =>
            await launch?.window.evaluate(async (gameName: string) => {
              const api = Reflect.get(window, "api") as
                | {
                    listEntries?: () => Promise<Array<{ id: string; title: string; type_id: string | null }>>;
                  }
                | undefined;
              const entries = api?.listEntries ? await api.listEntries() : [];
              return entries.find((entry) => entry.title === gameName) ?? null;
            }, TEST_GAME.name),
          { timeout: 20_000 },
        )
        .toMatchObject({
          title: TEST_GAME.name,
          type_id: "game_obj",
        });

      const gameEntry = launch.window.locator(".widget-nav-item", { hasText: TEST_GAME.name }).first();
      await expect(gameEntry).toBeVisible({ timeout: 20_000 });
      await gameEntry.click();

      await expect(launch.window.locator(".editor-wrapper")).toBeVisible();
      await expect(launch.window.locator(".title-input")).toHaveValue(TEST_GAME.name);
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
      }

      fs.rmSync(env.rootPath, { recursive: true, force: true });
      expect(fs.existsSync(env.rootPath)).toBe(false);
    }
  });
});
