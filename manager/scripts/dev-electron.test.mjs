import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

const manager = path.resolve(import.meta.dirname, "..");
const ALIVE_MS = 15_000;

const prerequisites =
  process.platform === "win32" &&
  existsSync(path.join(manager, "node_modules", "electron", "dist", "electron.exe")) &&
  existsSync(path.join(manager, "dist", "index.html")) &&
  existsSync(path.join(manager, "dist-electron", "main.js"));

// Replicates the manager dev pipeline's final stage: `pnpm exec electron .`
// must launch a process that stays alive instead of dying on EINVAL or on the
// single-instance lock held by an installed Kosmos. Requires manager deps and
// a build; in CI the first-party-contracts job provides both.
test(
  "pnpm exec electron launches the manager and keeps it alive",
  {
    timeout: 120_000,
    skip: prerequisites
      ? false
      : "requires Windows, manager deps, and a manager build (dist + dist-electron)",
  },
  async () => {
    const userDataDir = await mkdtemp(path.join(os.tmpdir(), "dev-electron-"));
    const child = spawn("pnpm.cmd", ["exec", "electron", `--user-data-dir=${userDataDir}`, "."], {
      cwd: manager,
      env: {
        ...process.env,
        KOSMOS_TEST_MODE: "1",
        KOSMOS_HEADLESS: "1",
        KOSMOS_DEV_PACKAGES: "1",
      },
      stdio: ["ignore", "pipe", "pipe"],
      shell: true,
    });
    let output = "";
    child.stdout.on("data", (chunk) => (output += chunk));
    child.stderr.on("data", (chunk) => (output += chunk));
    const exit = new Promise((resolve) => child.on("exit", (code) => resolve(code)));
    const result = await Promise.race([
      exit.then((code) => ({ exited: true, code })),
      new Promise((resolve) => setTimeout(() => resolve({ exited: false }), ALIVE_MS)),
    ]);
    spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
    assert.equal(
      result.exited,
      false,
      `electron exited early with code ${result.code}: ${output.slice(-2000)}`,
    );
  },
);
