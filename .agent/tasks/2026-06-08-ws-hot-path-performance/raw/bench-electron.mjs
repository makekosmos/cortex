import { _electron as electron } from "playwright";
import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const electronBinary = require("electron");
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");
const shellRoot = path.join(repoRoot, "platform", "desktop");
const taskRoot = path.resolve(__dirname, "..");
const label = process.argv[2] ?? "electron-after";
const flags = JSON.parse(process.argv[3] ?? "{}");
const dataDir = path.join(taskRoot, "smoke", label);
const userDataDir = path.join(dataDir, "electron-userdata");
const resultPath = path.join(__dirname, `${label}-electron-bench.json`);

fs.rmSync(dataDir, { recursive: true, force: true });
fs.mkdirSync(userDataDir, { recursive: true });

async function main() {
  const app = await electron.launch({
    executablePath: electronBinary,
    cwd: shellRoot,
    args: [path.join(shellRoot, "dist-electron", "main.js"), `--user-data-dir=${userDataDir}`],
    env: {
      ...process.env,
      NODE_ENV: "test",
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_TEST_MODE: "1",
      KOSMOS_HEADLESS: "1",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KEPLER_SKIP_SYNC: "1",
      KEPLER_USAGE_TRACKER: "0",
      KEPLER_FILE_INDEX_INITIAL_RESCAN: "0",
      ...flags,
    },
    timeout: 30_000,
  });

  try {
    const page = await app.firstWindow({ timeout: 20_000 });
    await page.waitForFunction(() => Boolean(window.kepler?.diagnostics), null, {
      timeout: 20_000,
    });
    const metrics = await page.evaluate(() => window.kepler.diagnostics.metrics());
    const flatMove = await page.evaluate(() =>
      window.kepler.diagnostics.windowMoveBenchmark({
        windowKind: "flatTest",
        steps: 240,
        intervalMs: 16,
      }),
    );
    const launcherMove = await page.evaluate(() =>
      window.kepler.diagnostics.windowMoveBenchmark({
        windowKind: "launcher",
        steps: 120,
        intervalMs: 16,
      }),
    );
    const result = {
      label,
      flags,
      dataDir,
      metrics,
      flatMove,
      launcherMove,
    };
    fs.writeFileSync(resultPath, JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result, null, 2));
  } finally {
    await app.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
