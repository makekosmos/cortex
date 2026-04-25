import { type ChildProcess, execFile, spawn } from "node:child_process";
import { createServer } from "node:net";
import { promisify } from "node:util";

const ROOT = process.cwd();
const HOST = "127.0.0.1";
const execFileAsync = promisify(execFile);

function run(command: string, args: string[], env = process.env) {
  return new Promise<void>((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: ROOT,
      env,
      shell: false,
      stdio: "inherit",
    });

    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) {
        resolve();
        return;
      }
      reject(new Error(`${command} ${args.join(" ")} exited with code ${code}`));
    });
  });
}

async function runE2ESuite(env: NodeJS.ProcessEnv) {
  try {
    await run(process.execPath, ["x", "playwright", "test"], env);
  } catch (error) {
    if (env.ARRANCADOR_E2E_INLINE_FALLBACK !== "1") {
      throw error;
    }
    console.warn(
      "[e2e] Playwright test runner failed, retrying with inline fallback:",
      error,
    );
    await run(process.execPath, ["run", "scripts/run-e2e-inline.ts"], env);
  }
}

function getAvailablePort() {
  return new Promise<number>((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, HOST, () => {
      const address = server.address();
      server.close(() => {
        if (address && typeof address === "object") {
          resolve(address.port);
          return;
        }
        reject(new Error("Failed to allocate a preview port"));
      });
    });
  });
}

async function waitForPreview(url: string, hasPreviewExited: () => boolean) {
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    if (hasPreviewExited()) {
      throw new Error(`Preview server exited before becoming ready at ${url}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
    } catch {
      // Preview is not ready yet.
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`Preview server did not become ready at ${url}`);
}

async function waitForExit(child: ChildProcess, timeoutMs: number) {
  await new Promise<void>((resolve) => {
    const timeout = setTimeout(resolve, timeoutMs);
    child.once("exit", () => {
      clearTimeout(timeout);
      resolve();
    });
  });
}

async function stopPreview(child: ChildProcess, hasExited: () => boolean) {
  if (hasExited()) {
    return;
  }

  if (process.platform === "win32" && child.pid) {
    try {
      await execFileAsync("taskkill", ["/PID", String(child.pid), "/T", "/F"]);
    } catch {
      // Sandbox environments may block taskkill after the browser spawn fails.
    }
  } else if (!child.killed) {
    child.kill();
  }

  await waitForExit(child, 3_000);
}

await run(process.execPath, ["run", "build"]);

const port = await getAvailablePort();
const previewUrl = `http://${HOST}:${port}`;
let previewExited = false;
const preview = spawn(
  process.execPath,
  [
    "x",
    "vite",
    "preview",
    "--configLoader",
    "native",
    "--host",
    HOST,
    "--port",
    String(port),
    "--strictPort",
  ],
  {
    cwd: ROOT,
    env: process.env,
    shell: false,
    stdio: "inherit",
  },
);

preview.once("exit", () => {
  previewExited = true;
});

try {
  await waitForPreview(previewUrl, () => previewExited);
  await runE2ESuite({
    ...process.env,
    ARRANCADOR_E2E_BASE_URL: previewUrl,
    ARRANCADOR_E2E_EXTERNAL_SERVER: "1",
  });
} finally {
  await stopPreview(preview, () => previewExited);
}
