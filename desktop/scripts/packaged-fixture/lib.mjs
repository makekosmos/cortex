// Shared helpers for the KOS-58 packaged smoke fixture. See
// ../packaged-fixture.mjs for the scenario list and usage.
import { execFileSync, spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { processInfo } from "../dev-run-process.mjs";

export const HERE = path.dirname(fileURLToPath(import.meta.url));
export const DESKTOP = path.resolve(HERE, "..", "..");
export const REPO = path.resolve(DESKTOP, "..");
export const DEFAULT_OUT = path.join(DESKTOP, ".e2e", "packaged-fixture");
const FIXTURE_MANIFEST = "fixture-manifest.json";
// Fixture-local Engine version used when binaries are overridden. The value
// must stay semver and must never collide with a published Engine release.
export const FIXTURE_ENGINE_VERSION = "0.1.90";
export const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
export const iso = () => new Date().toISOString();

// ---------------------------------------------------------------------------
// args / io helpers
// ---------------------------------------------------------------------------
export function parseArgs(argv) {
  const args = { _: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) {
      args._.push(token);
      continue;
    }
    const eq = token.indexOf("=");
    if (eq !== -1) {
      args[token.slice(2, eq)] = token.slice(eq + 1);
    } else if (argv[index + 1] !== undefined && !argv[index + 1].startsWith("--")) {
      args[token.slice(2)] = argv[(index += 1)];
    } else {
      args[token.slice(2)] = true;
    }
  }
  return args;
}

const sha256File = (file) =>
  crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");

export const readJson = (file) => JSON.parse(fs.readFileSync(file, "utf8"));
export const writeJson = (file, value) => {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
};
export const ensureDir = (dir) => fs.mkdirSync(dir, { recursive: true });

export function artifactRecord(file, extra = {}) {
  return {
    path: path.resolve(file),
    sha256: sha256File(file),
    size: fs.statSync(file).size,
    ...extra,
  };
}

export function ps(command, { env = process.env, timeout = 15_000 } = {}) {
  return execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", command], {
    encoding: "utf8",
    windowsHide: true,
    env,
    timeout,
  }).trim();
}

export function psJson(command, options) {
  const output = ps(`${command} | ConvertTo-Json -Compress`, options);
  if (!output) return [];
  const parsed = JSON.parse(output);
  return Array.isArray(parsed) ? parsed : [parsed];
}

export function processesUnder(roots) {
  const list = roots.map((root) => path.resolve(root));
  return psJson(
    "@(Get-CimInstance Win32_Process | Where-Object {" +
      "$_.ExecutablePath -and (" +
      list
        .map(
          (root) =>
            `$_.ExecutablePath.StartsWith('${root.replace(/'/g, "''")}',[StringComparison]::OrdinalIgnoreCase)`,
        )
        .join(" -or ") +
      ")} | Select-Object ProcessId,ExecutablePath,CommandLine,CreationDate)",
  );
}

export async function waitFor(read, label, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const value = await read();
      if (value !== undefined && value !== null && value !== false) return value;
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }
  throw new Error(`timed out waiting for ${label}${lastError ? ` (${lastError.message})` : ""}`);
}

export async function freeTcpPort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

// Spawned fixture processes inherit the session environment (like
// candidate-launcher-smoke.mjs) with per-run isolation overrides on top.
export function fixtureEnv(overrides = {}) {
  const env = { ...process.env };
  for (const [key, value] of Object.entries(overrides)) {
    if (value === undefined) delete env[key];
    else env[key] = value;
  }
  return env;
}

export function spawnLogged(exe, args, { env, cwd, logFile, detached = false }) {
  ensureDir(path.dirname(logFile));
  const stream = fs.createWriteStream(logFile, { flags: "a" });
  stream.write(`\n===== ${iso()} spawn ${exe} ${args.join(" ")}\n`);
  const child = spawn(exe, args, {
    env,
    cwd,
    detached,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  child.stdout?.on("data", (chunk) => stream.write(chunk));
  child.stderr?.on("data", (chunk) => stream.write(chunk));
  const exited = new Promise((resolve) => {
    child.once("exit", (code, signal) => {
      stream.end(`\n===== ${iso()} exit code=${code} signal=${signal}\n`);
      resolve({ code, signal });
    });
    child.once("error", (error) => {
      stream.end(`\n===== ${iso()} spawn error ${error.message}\n`);
      resolve({ code: null, signal: null, error: error.message });
    });
  });
  return { child, exited };
}

// ---------------------------------------------------------------------------
// engine / RPC helpers (mirrors host/e2e/fixtures/host-runtime.ts conventions)
// ---------------------------------------------------------------------------
export function waitForEngineLock(dataDir, timeout = 30_000) {
  const lockPath = path.join(dataDir, "engine.lock.json");
  return waitFor(
    () => {
      try {
        // SAFETY: this fixture-owned path is written only by the Engine lock serializer.
        const lock = readJson(lockPath);
        if (Number.isInteger(lock.pid) && processInfo(lock.pid)) return lock;
      } catch {}
      return undefined;
    },
    "Engine lock",
    timeout,
  );
}

export async function engineRpc(lock, operation, params = {}) {
  const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      Connection: "close",
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "desktop-host",
      "X-Kosmos-Client-Version": "1.0.0",
      "X-Kosmos-Client-Pid": String(process.pid),
    },
    body: JSON.stringify({ operation, _req_id: crypto.randomUUID(), ...params }),
    signal: AbortSignal.timeout(20_000),
  });
  return response.json();
}

export function engineShutdown(engineBackend, dataDir) {
  try {
    execFileSync(engineBackend, ["--shutdown"], {
      env: fixtureEnv({
        KOSMOS_DATA_DIR: dataDir,
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      }),
      windowsHide: true,
      stdio: "ignore",
      timeout: 10_000,
    });
  } catch {}
}

export async function waitPidGone(pid, label, timeout = 10_000) {
  const deadline = Date.now() + timeout;
  while (processInfo(pid) && Date.now() < deadline) await delay(100);
  if (processInfo(pid)) throw new Error(`${label} PID ${pid} is still alive`);
}

// ---------------------------------------------------------------------------
// fixture manifest / layout
// ---------------------------------------------------------------------------
export function layout(outDir) {
  return {
    out: outDir,
    manifest: path.join(outDir, FIXTURE_MANIFEST),
    inputs: path.join(outDir, "inputs"),
    engineRoot: path.join(outDir, "engine"),
    runs: path.join(outDir, "runs"),
    reportJson: path.join(outDir, "report.json"),
    reportMd: path.join(outDir, "REPORT.md"),
  };
}

export function loadFixture(outDir) {
  const file = path.join(outDir, FIXTURE_MANIFEST);
  if (!fs.existsSync(file))
    throw new Error(`fixture manifest missing: ${file} (run 'prepare' first)`);
  return readJson(file);
}

export function catalogEntries(catalogEnvelope) {
  const bytes = Buffer.from(catalogEnvelope.bytes, "base64").toString("utf8");
  const document = JSON.parse(bytes);
  const packages = document.packages ?? [];
  return { document, packages };
}

export function gitRevision() {
  try {
    const sha = execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: REPO,
      encoding: "utf8",
    }).trim();
    const dirty = execFileSync("git", ["status", "--porcelain"], {
      cwd: REPO,
      encoding: "utf8",
    })
      .split("\n")
      .filter(Boolean).length;
    return { sha, dirtyEntries: dirty };
  } catch {
    return { sha: null, dirtyEntries: null };
  }
}

// Uniform check recorder shared by every scenario.
export function makeCheck(result) {
  return (name, ok, detail = undefined) => {
    const entry = { name, status: ok ? "pass" : "fail" };
    if (detail) entry.detail = detail;
    result.checks.push(entry);
    return ok;
  };
}

export function scenarioNotRun(outDir, name, reason) {
  const runDir = path.join(outDir, "runs", name);
  ensureDir(runDir);
  const result = {
    scenario: name,
    started_at: iso(),
    finished_at: iso(),
    status: "NOT_RUN",
    reason,
    checks: [],
  };
  writeJson(path.join(runDir, "result.json"), result);
  console.log(JSON.stringify({ scenario: name, status: "NOT_RUN", reason }));
}
