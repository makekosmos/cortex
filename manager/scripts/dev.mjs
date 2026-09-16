import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import net from "node:net";

const manager = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cortex = path.resolve(manager, "..");
const config = JSON.parse(fs.readFileSync(path.join(cortex, "dev-packages.json"), "utf8"));
const packages = Array.isArray(config.packages) ? config.packages : [];
const env = { ...process.env, KOSMOS_DEV_PACKAGES: "1", KOSMOS_MANAGER_EXTERNAL_ELECTRON: "1" };
const buildEnv = { ...env };
delete buildEnv.KOSMOS_MANAGER_EXTERNAL_ELECTRON;
const available = (port) =>
  new Promise((resolve) => {
    const server = net.createServer();
    server.once("error", () => resolve(false));
    server.once("listening", () => server.close(() => resolve(true)));
    server.listen(port, "127.0.0.1");
  });
const pnpm = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
const run = (cwd, args, inherited = env) => {
  const result = spawnSync(pnpm, args, {
    cwd,
    env: inherited,
    stdio: "inherit",
    shell: pnpm.endsWith(".cmd"),
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
};
const start = (cwd, args, inherited = env) => {
  const child = spawn(pnpm, args, {
    cwd,
    env: inherited,
    stdio: "inherit",
    shell: pnpm.endsWith(".cmd"),
  });
  child.on("exit", (code) => process.exit(code ?? 0));
  return child;
};

if (!(await available(5174))) {
  console.error("Port 5174 is already in use. Stop the existing Manager dev session first.");
  process.exit(1);
}

run(path.join(cortex, "host"), ["run", "build"]);
run(manager, ["run", "build"], buildEnv);
for (const entry of packages) {
  const relativePath = String(entry?.path ?? "").trim();
  if (!relativePath) continue;
  const cwd = path.resolve(cortex, relativePath);
  run(cwd, ["run", "package:kspkg"]);
  start(cwd, ["run", "dev"]);
}
start(manager, ["exec", "vite", "--host", "127.0.0.1", "--port", "5174", "--strictPort"]);
setTimeout(
  () =>
    start(manager, ["exec", "electron", "."], {
      ...env,
      VITE_DEV_SERVER_URL: "http://127.0.0.1:5174",
      KOSMOS_HOST_MAIN: path.join(cortex, "host", "dist-electron", "main.js"),
    }),
  800,
);
