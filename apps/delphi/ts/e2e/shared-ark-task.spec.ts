import { test, expect, _electron as electron, type Page } from "@playwright/test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createRequire as createNodeRequire } from "node:module";
import { fileURLToPath } from "node:url";
import crypto from "node:crypto";

type ElectronApp = Awaited<ReturnType<typeof electron.launch>>;

interface LaunchedApp {
  electronApp: ElectronApp;
  window: Page;
  pageErrors: string[];
}

interface AppEnvironment {
  rootPath: string;
  homePath: string;
  userDataPath: string;
  appDataPath: string;
  localAppDataPath: string;
  vaultPath: string;
  arkDbPath: string;
  startupLogPath: string;
}

const TEST_TASK = {
  title: "Shared Ark Delphi Task",
  notes: "Created in Delphi and expected to be visible in Eden.",
};

const requireFromDelphi = createNodeRequire(new URL("../package.json", import.meta.url));
const delphiElectronExecutable = requireFromDelphi("electron") as string;
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

interface SharedSelectedSpace {
  version: 1;
  spaceCode: string;
  spaceId: string;
  vaultPath: string | null;
  source: string;
  updatedAt: string;
}

function normalizeSpaceCode(code: string): string {
  return code.replace(/[-\s]/g, "").toUpperCase();
}

function normalizeVaultPath(vaultPath: string): string {
  const resolved = path.resolve(vaultPath).replaceAll("/", "\\");
  return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

function deriveSpaceIdFromCode(code: string): string {
  return crypto
    .createHash("sha256")
    .update(normalizeSpaceCode(code), "utf8")
    .digest("hex")
    .slice(0, 16);
}

function derivePersonalSpaceCodeFromVaultPath(vaultPath: string): string {
  const crockford = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  const digest = crypto
    .createHash("sha256")
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

function buildPersonalSelectedSpace(
  vaultPath: string,
  source: string,
): SharedSelectedSpace {
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

function writeSelectedSpace(
  appDataPath: string,
  selection: SharedSelectedSpace | null,
): void {
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
  const rootPath = fs.mkdtempSync(path.join(os.tmpdir(), "delphi-ark-task-"));
  const homePath = path.join(rootPath, "home");
  const userDataPath = path.join(rootPath, "userData");
  const appDataPath = path.join(rootPath, "appdata", "Roaming");
  const localAppDataPath = path.join(rootPath, "appdata", "Local");
  const vaultPath = path.join(rootPath, "vault-main");

  fs.mkdirSync(homePath, { recursive: true });
  fs.mkdirSync(userDataPath, { recursive: true });
  fs.mkdirSync(appDataPath, { recursive: true });
  fs.mkdirSync(localAppDataPath, { recursive: true });
  fs.mkdirSync(vaultPath, { recursive: true });

  const selection = buildPersonalSelectedSpace(vaultPath, "delphi-e2e");
  writeSelectedSpace(appDataPath, selection);

  return {
    rootPath,
    homePath,
    userDataPath,
    appDataPath,
    localAppDataPath,
    vaultPath,
    arkDbPath: getArkDbPathForSelectedSpace(appDataPath, selection),
    startupLogPath: path.join(rootPath, "delphi-startup.log"),
  };
}

async function launchElectronApp(
  appPath: string,
  env: AppEnvironment,
  extraEnv: Record<string, string> = {},
  attempt = 0,
): Promise<LaunchedApp> {
  let electronApp: ElectronApp;
  try {
    electronApp = await electron.launch({
      executablePath: appPath.endsWith(path.join("delphi", "ts"))
        ? delphiElectronExecutable
        : undefined,
      cwd: appPath,
      args: ["."],
      env: {
        ...process.env,
        HOME: env.homePath,
        APPDATA: env.appDataPath,
        LOCALAPPDATA: env.localAppDataPath,
        KEPLER_TEST_APPDATA: env.appDataPath,
        KEPLER_TEST_USER_DATA: env.userDataPath,
        DELPHI_STARTUP_LOG: env.startupLogPath,
        PLAYWRIGHT: "1",
        ...extraEnv,
      },
    });
  } catch (error) {
    const startupLog = fs.existsSync(env.startupLogPath)
      ? fs.readFileSync(env.startupLogPath, "utf8")
      : "";
    throw new Error(`${error instanceof Error ? error.message : String(error)}\nStartup log:\n${startupLog}`);
  }

  try {
    const window = await electronApp.firstWindow({ timeout: 45_000 });
    const pageErrors: string[] = [];

    window.on("pageerror", (error) => {
      pageErrors.push(error.message);
    });

    await window.waitForLoadState("domcontentloaded");

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

    return launchElectronApp(appPath, env, extraEnv, attempt + 1);
  }
}

async function launchDelphi(env: AppEnvironment): Promise<LaunchedApp> {
  const appPath = path.resolve(__dirname, "..");
  const launched = await launchElectronApp(appPath, env, {
    DELPHI_BACKGROUND_LAUNCH: "1",
    NODE_ENV: "production",
  });

  await launched.window.waitForSelector('button[title*="задач"]', {
    timeout: 20_000,
  });

  return launched;
}

async function launchEden(env: AppEnvironment): Promise<LaunchedApp> {
  const appPath = path.resolve(__dirname, "../../../eden/ts");
  const launched = await launchElectronApp(appPath, env, {
    EDEN_BACKGROUND_LAUNCH: "1",
    NODE_ENV: "production",
  });

  await launched.window.waitForSelector(".app-container", { timeout: 20_000 });

  return launched;
}

async function createTaskInDelphi(window: Page) {
  const createButton = window.locator('button[title*="задач"]').first();
  await createButton.click();

  const titleInput = window.getByPlaceholder("Новая задача");
  await titleInput.fill(TEST_TASK.title);

  const notesInput = window.getByPlaceholder("Заметки");
  await notesInput.fill(TEST_TASK.notes);

  await titleInput.focus();
  await titleInput.press("Enter");

  await expect(window.getByText(TEST_TASK.title)).toBeVisible({ timeout: 20_000 });
}

async function findDelphiArkTask(window: Page) {
  return window.evaluate(async (taskTitle: string) => {
    const api = Reflect.get(window, "electronAPI") as
      | { invoke?: (channel: string, ...args: unknown[]) => Promise<unknown> }
      | undefined;
    const tasks = api?.invoke ? await api.invoke("ark:listDelphiTasks") : [];
    return Array.isArray(tasks)
      ? tasks.find((task) => {
          const candidate = task as { title?: unknown };
          return candidate.title === taskTitle;
        }) ?? null
      : null;
  }, TEST_TASK.title);
}

test.describe("Delphi shared Ark task objects", () => {
  test.describe.configure({ mode: "serial" });

  test("writes a Delphi task into the shared Ark DB and Eden can open it", async () => {
    test.setTimeout(120_000);

    const env = createEnvironment();
    let delphi: LaunchedApp | null = null;
    let eden: LaunchedApp | null = null;

    try {
      delphi = await launchDelphi(env);

      await expect(
        delphi.window.getByText("Ark Space"),
      ).toHaveCount(0);

      await createTaskInDelphi(delphi.window);

      await expect
        .poll(() => fs.existsSync(env.arkDbPath), { timeout: 20_000 })
        .toBe(true);

      await expect
        .poll(() => findDelphiArkTask(delphi.window), { timeout: 20_000 })
        .toMatchObject({
          title: TEST_TASK.title,
          notes: TEST_TASK.notes,
        });

      eden = await launchEden(env);

      await expect
        .poll(
          async () =>
            await eden?.window.evaluate(async (taskTitle: string) => {
              const api = Reflect.get(window, "api") as
                | {
                    listEntries?: () => Promise<
                      Array<{ id: string; title: string; type_id: string | null }>
                    >;
                  }
                | undefined;
              const entries = api?.listEntries ? await api.listEntries() : [];
              return entries.find((entry) => entry.title === taskTitle) ?? null;
            }, TEST_TASK.title),
          { timeout: 20_000 },
        )
        .toMatchObject({
          title: TEST_TASK.title,
          type_id: "task_obj",
        });

      const taskEntry = eden.window
        .locator(".widget-nav-item", { hasText: TEST_TASK.title })
        .first();
      await expect(taskEntry).toBeVisible({ timeout: 20_000 });
      await taskEntry.click();

      await expect(eden.window.locator(".editor-wrapper")).toBeVisible({
        timeout: 20_000,
      });
      await expect(eden.window.locator(".title-input")).toHaveValue(TEST_TASK.title);

      expect(delphi.pageErrors).toEqual([]);
      expect(eden.pageErrors).toEqual([]);
    } finally {
      if (delphi) {
        await delphi.electronApp.close();
      }
      if (eden) {
        await eden.electronApp.close();
      }
      fs.rmSync(env.rootPath, { recursive: true, force: true });
    }
  });
});
