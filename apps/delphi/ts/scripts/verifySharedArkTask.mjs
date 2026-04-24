import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createRequire } from "node:module";
import { _electron as electron } from "playwright";

const require = createRequire(new URL("../../../arrancador/package.json", import.meta.url));
const BetterSqlite3 = require("better-sqlite3");
const crypto = require("node:crypto");

const TEST_TASK = {
  title: "Shared Ark Delphi Task",
  notes: "Created in Delphi and expected to be visible in Eden.",
};

function normalizeSpaceCode(code) {
  return code.replace(/[-\s]/g, "").toUpperCase();
}

function normalizeVaultPath(vaultPath) {
  const resolved = path.resolve(vaultPath).replaceAll("/", "\\");
  return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

function deriveSpaceIdFromCode(code) {
  return crypto
    .createHash("sha256")
    .update(normalizeSpaceCode(code), "utf8")
    .digest("hex")
    .slice(0, 16);
}

function derivePersonalSpaceCodeFromVaultPath(vaultPath) {
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

function buildPersonalSelectedSpace(vaultPath, source) {
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

function writeSelectedSpace(appDataPath, selection) {
  const filePath = path.join(appDataPath, "Kepler", "selected-space.json");
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(selection, null, 2), "utf8");
}

function getArkDbPathForSelectedSpace(appDataPath, selection) {
  const dataDir = path.join(appDataPath, "Kepler");
  return selection?.spaceId
    ? path.join(dataDir, "spaces", selection.spaceId, "ark.db")
    : path.join(dataDir, "ark.db");
}

function createEnvironment() {
  const rootPath = fs.mkdtempSync(path.join(os.tmpdir(), "delphi-ark-task-"));
  const homePath = path.join(rootPath, "home");
  const appDataPath = path.join(rootPath, "appdata", "Roaming");
  const localAppDataPath = path.join(rootPath, "appdata", "Local");
  const vaultPath = path.join(rootPath, "vault-main");

  fs.mkdirSync(homePath, { recursive: true });
  fs.mkdirSync(appDataPath, { recursive: true });
  fs.mkdirSync(localAppDataPath, { recursive: true });
  fs.mkdirSync(vaultPath, { recursive: true });

  const selection = buildPersonalSelectedSpace(vaultPath, "delphi-e2e");
  writeSelectedSpace(appDataPath, selection);

  return {
    rootPath,
    homePath,
    appDataPath,
    localAppDataPath,
    vaultPath,
    arkDbPath: getArkDbPathForSelectedSpace(appDataPath, selection),
  };
}

async function waitFor(predicate, { timeoutMs = 20000, intervalMs = 250, errorMessage }) {
  const deadline = Date.now() + timeoutMs;
  let lastValue = null;

  while (Date.now() < deadline) {
    lastValue = await predicate();
    if (lastValue) {
      return lastValue;
    }
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }

  throw new Error(errorMessage ?? `Timed out waiting for condition. Last value: ${String(lastValue)}`);
}

async function launchElectronApp(appPath, env, extraEnv = {}) {
  const electronApp = await electron.launch({
    args: [appPath],
    env: {
      ...process.env,
      HOME: env.homePath,
      USERPROFILE: env.homePath,
      APPDATA: env.appDataPath,
      LOCALAPPDATA: env.localAppDataPath,
      PLAYWRIGHT: "1",
      ...extraEnv,
    },
  });

  const window = await electronApp.firstWindow({ timeout: 45000 });
  const pageErrors = [];
  window.on("pageerror", (error) => {
    pageErrors.push(error.message);
  });

  await window.waitForLoadState("domcontentloaded");

  return { electronApp, window, pageErrors };
}

async function launchDelphi(env) {
  const appPath = path.resolve("D:/Personal/Hobby/Coding/kepler/apps/delphi/ts");
  const launched = await launchElectronApp(appPath, env, {
    DELPHI_BACKGROUND_LAUNCH: "1",
    NODE_ENV: "production",
  });
  await launched.window.waitForSelector('button[title*="задач"]', { timeout: 20000 });
  return launched;
}

async function launchEden(env) {
  const appPath = path.resolve("D:/Personal/Hobby/Coding/kepler/apps/eden/ts");
  const launched = await launchElectronApp(appPath, env, {
    EDEN_BACKGROUND_LAUNCH: "1",
    NODE_ENV: "production",
  });
  await launched.window.waitForSelector(".app-container", { timeout: 20000 });
  return launched;
}

function readArkTaskRow(arkDbPath) {
  const db = new BetterSqlite3(arkDbPath, {
    readonly: true,
    fileMustExist: true,
  });

  try {
    return (
      db
        .prepare(
          `SELECT id, type_id, title, props_json, content_json
           FROM objects
           WHERE type_id = ?1 AND title = ?2 AND deleted_at IS NULL
           LIMIT 1`,
        )
        .get("task_obj", TEST_TASK.title) ?? null
    );
  } finally {
    db.close();
  }
}

function readArkTaskObjectTypeExists(arkDbPath) {
  const db = new BetterSqlite3(arkDbPath, {
    readonly: true,
    fileMustExist: true,
  });

  try {
    const row = db
      .prepare("SELECT id FROM object_types WHERE id = ?1 LIMIT 1")
      .get("task_obj");
    return Boolean(row?.id);
  } finally {
    db.close();
  }
}

async function createTaskInDelphi(window) {
  await window.locator('button[title*="задач"]').first().click();
  await window.getByPlaceholder("Новая задача").fill(TEST_TASK.title);
  await window.getByPlaceholder("Заметки").fill(TEST_TASK.notes);
  await window.getByPlaceholder("Новая задача").focus();
  await window.getByPlaceholder("Новая задача").press("Enter");
  await window.getByText(TEST_TASK.title).waitFor({ timeout: 20000 });
}

async function main() {
  const env = createEnvironment();
  let delphi = null;
  let eden = null;

  try {
    delphi = await launchDelphi(env);
    assert.equal(await delphi.window.getByText("Ark Space").count(), 0);

    await createTaskInDelphi(delphi.window);

    await waitFor(() => fs.existsSync(env.arkDbPath), {
      errorMessage: `Ark DB was not created at ${env.arkDbPath}`,
    });

    await waitFor(() => readArkTaskObjectTypeExists(env.arkDbPath), {
      errorMessage: "task_obj object type was not written to Ark DB",
    });

    const row = await waitFor(() => readArkTaskRow(env.arkDbPath), {
      errorMessage: "Delphi task object was not found in Ark DB",
    });

    assert.equal(row.type_id, "task_obj");
    assert.equal(row.title, TEST_TASK.title);

    const props = JSON.parse(row.props_json ?? "{}");
    assert.equal(props.description, TEST_TASK.notes);
    assert.equal(props.source_app, "delphi");

    eden = await launchEden(env);

    const entry = await waitFor(
      () =>
        eden.window.evaluate(async (taskTitle) => {
          const api = Reflect.get(window, "api");
          if (!api || typeof api !== "object" || typeof api.listEntries !== "function") {
            return null;
          }
          const entries = await api.listEntries();
          return entries.find((item) => item.title === taskTitle) ?? null;
        }, TEST_TASK.title),
      {
        errorMessage: "Eden did not expose the Delphi task from the shared Ark DB",
      },
    );

    assert.equal(entry.title, TEST_TASK.title);
    assert.equal(entry.type_id, "task_obj");

    const taskEntry = eden.window.locator(".widget-nav-item", {
      hasText: TEST_TASK.title,
    }).first();
    await taskEntry.waitFor({ timeout: 20000 });
    await taskEntry.click();

    await eden.window.locator(".editor-wrapper").waitFor({ timeout: 20000 });
    assert.equal(await eden.window.locator(".title-input").inputValue(), TEST_TASK.title);

    assert.deepEqual(delphi.pageErrors, []);
    assert.deepEqual(eden.pageErrors, []);

    console.log("VERIFY_SHARED_ARK_TASK: PASS");
    console.log(`ARK_DB_PATH=${env.arkDbPath}`);
  } finally {
    if (delphi) {
      await delphi.electronApp.close();
    }
    if (eden) {
      await eden.electronApp.close();
    }
    fs.rmSync(env.rootPath, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error("VERIFY_SHARED_ARK_TASK: FAIL");
  console.error(error);
  process.exitCode = 1;
});
