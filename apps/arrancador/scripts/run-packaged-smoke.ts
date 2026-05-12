import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { _electron as electron } from "playwright";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const smokeRoot = path.join(appRoot, ".e2e", "packaged-smoke");
const appDataRoot = path.join(smokeRoot, "appdata");
const localAppDataRoot = path.join(smokeRoot, "localappdata");
const arkDbPath = path.join(smokeRoot, "ark", "ark.db");

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function findPackagedExe() {
  const releaseRoot = path.join(appRoot, "release");
  const preferred = path.join(releaseRoot, "win-unpacked", "arrancador.exe");
  if (fs.existsSync(preferred)) {
    return preferred;
  }

  const pending = [releaseRoot];
  while (pending.length > 0) {
    const current = pending.pop();
    if (!current || !fs.existsSync(current)) {
      continue;
    }

    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const entryPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        pending.push(entryPath);
        continue;
      }

      if (entry.isFile() && entry.name.toLowerCase() === "arrancador.exe") {
        return entryPath;
      }
    }
  }

  throw new Error(`Packaged Arrancador executable was not found under ${releaseRoot}`);
}

fs.mkdirSync(path.dirname(arkDbPath), { recursive: true });
fs.mkdirSync(appDataRoot, { recursive: true });
fs.mkdirSync(localAppDataRoot, { recursive: true });

const executablePath = findPackagedExe();
const app = await electron.launch({
  executablePath,
  cwd: appRoot,
  env: {
    ...process.env,
    APPDATA: appDataRoot,
    LOCALAPPDATA: localAppDataRoot,
    ARK_DB_PATH: arkDbPath,
    NODE_ENV: "test",
    ELECTRON_ENABLE_LOGGING: "1",
  },
});

try {
  const page = await app.firstWindow({ timeout: 20_000 });
  await page.waitForLoadState("domcontentloaded", { timeout: 20_000 });
  await page.waitForFunction(() => document.body?.textContent && document.body.textContent.length > 0, {
    timeout: 20_000,
  });

  const title = await page.title();
  const bodyLength = await page.evaluate(() => document.body?.textContent?.length ?? 0);
  const runtimePaths = await app.evaluate(async ({ app: electronApp }) => ({
    appData: electronApp.getPath("appData"),
    userData: electronApp.getPath("userData"),
  }));

  assert(title.includes("Arrancador"), `Unexpected page title: ${title}`);
  assert(bodyLength > 0, "Renderer body is empty");
  assert(
    String(runtimePaths.userData).startsWith(localAppDataRoot),
    `User data path is not isolated: ${runtimePaths.userData}`,
  );

  console.log(
    JSON.stringify(
      {
        status: "ok",
        executablePath,
        appData: runtimePaths.appData,
        userData: runtimePaths.userData,
        arkDbPath,
        title,
        bodyLength,
      },
      null,
      2,
    ),
  );
} finally {
  await app.close();
}
