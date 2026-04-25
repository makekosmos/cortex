import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const repoRoot = process.cwd();
const binaryPath = path.join(
  repoRoot,
  "apps",
  "delphi",
  "ts",
  "release",
  "win-unpacked",
  "resources",
  "ark-core",
  process.platform === "win32" ? "ark-core-rpc.exe" : "ark-core-rpc",
);

if (!fs.existsSync(binaryPath)) {
  throw new Error(`Packaged ark-core-rpc not found at ${binaryPath}`);
}

const taskDir = path.join(
  repoRoot,
  ".agent",
  "tasks",
  "2026-04-25-ark-object-first-auto-migrations",
  "raw",
);
fs.mkdirSync(taskDir, { recursive: true });
const dbPath = path.join(taskDir, `delphi-packaged-smoke-${Date.now()}.db`);
const child = spawn(binaryPath, [], {
  cwd: repoRoot,
  stdio: ["pipe", "pipe", "pipe"],
});

const pending = new Map();
let stdoutBuffer = "";
let stderrBuffer = "";

child.stdout.setEncoding("utf8");
child.stdout.on("data", (chunk) => {
  stdoutBuffer += chunk;
  let newlineIndex = stdoutBuffer.indexOf("\n");
  while (newlineIndex !== -1) {
    const line = stdoutBuffer.slice(0, newlineIndex).trim();
    stdoutBuffer = stdoutBuffer.slice(newlineIndex + 1);
    if (line.length > 0) {
      const message = JSON.parse(line);
      if (message.id && pending.has(message.id)) {
        pending.get(message.id)(message);
        pending.delete(message.id);
      }
    }
    newlineIndex = stdoutBuffer.indexOf("\n");
  }
});

child.stderr.setEncoding("utf8");
child.stderr.on("data", (chunk) => {
  stderrBuffer += chunk;
});

let sequence = 0;
function request(operation, payload = {}) {
  const id = `packaged-smoke-${++sequence}`;
  const line = JSON.stringify({ id, operation, ...payload });
  child.stdin.write(`${line}${os.EOL}`);
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`Timed out waiting for ${operation}. stderr=${stderrBuffer}`));
    }, 10_000);
    pending.set(id, (message) => {
      clearTimeout(timeout);
      if (message.error) {
        reject(new Error(`${operation} failed: ${message.error}`));
      } else {
        resolve(message);
      }
    });
  });
}

try {
  await request("init", { dbPath });
  const listResponse = await request("list_objects");
  console.log(
    JSON.stringify(
      {
        binaryPath,
        dbPath,
        listObjectsOk: listResponse.ok === true,
        itemCount: Array.isArray(listResponse.data) ? listResponse.data.length : null,
      },
      null,
      2,
    ),
  );
} finally {
  child.stdin.end();
  child.kill();
}
