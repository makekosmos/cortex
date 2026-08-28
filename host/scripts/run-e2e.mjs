import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const hostRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(hostRoot, "..");
const tempRoot = path.resolve(os.tmpdir());
const manifestPath = path.join(repositoryRoot, ".tmp", `host-e2e-cleanup-${randomUUID()}.json`);
fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
fs.writeFileSync(manifestPath, JSON.stringify({ roots: [], pids: [] }));

let exited = 1;
try {
  exited = await new Promise((resolve) => {
    const child = spawn(
      process.execPath,
      ["x", "playwright", "test", "--config", "playwright.config.ts", ...process.argv.slice(2)],
      {
        cwd: hostRoot,
        env: { ...process.env, KOSMOS_HOST_E2E_CLEANUP_MANIFEST: manifestPath },
        stdio: "inherit",
        windowsHide: true,
      },
    );
    child.once("error", (error) => {
      console.error(`[host-e2e] runner failed: ${error.message}`);
      resolve(1);
    });
    child.once("exit", (code) => resolve(code ?? 1));
  });
} catch (error) {
  console.error(
    `[host-e2e] runner failed: ${error instanceof Error ? error.message : String(error)}`,
  );
}

let cleanupFailed = false;
const isString = (value) => value?.constructor === String;
try {
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  if (!Array.isArray(manifest.roots) || !Array.isArray(manifest.pids))
    throw new Error("invalid cleanup manifest");
  for (const root of manifest.roots) {
    const resolved = isString(root) ? path.resolve(root) : "";
    if (
      path.dirname(resolved).toLowerCase() !== tempRoot.toLowerCase() ||
      !path.basename(resolved).startsWith("kosmos-host-e2e-")
    )
      throw new Error(`unsafe cleanup root: ${root}`);
    fs.rmSync(resolved, { recursive: true, force: true, maxRetries: 50, retryDelay: 100 });
    if (fs.existsSync(resolved)) throw new Error(`cleanup root remains: ${resolved}`);
    console.log(`[host-e2e] cleaned ${resolved}`);
  }
  for (const pid of manifest.pids) {
    if (!Number.isInteger(pid) || pid <= 0) throw new Error(`invalid cleanup PID: ${pid}`);
    try {
      process.kill(pid, 0);
      throw new Error(`cleanup PID remains alive: ${pid}`);
    } catch (error) {
      if (error instanceof Error && error.message.startsWith("cleanup PID")) throw error;
    }
  }
} catch (error) {
  cleanupFailed = true;
  console.error(
    `[host-e2e] cleanup failed: ${error instanceof Error ? error.message : String(error)}`,
  );
} finally {
  fs.rmSync(manifestPath, { force: true });
}

process.exitCode = exited === 0 && !cleanupFailed ? 0 : 1;
