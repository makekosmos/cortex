import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = fileURLToPath(new URL("..", import.meta.url));
const outDir = path.join(appRoot, ".tmp", "bench");
const outFile = path.join(outDir, "run-baseline.mjs");
mkdirSync(outDir, { recursive: true });

const build = spawnSync(
  "bun",
  [
    "build",
    "./bench/run-baseline.ts",
    `--outfile=${outFile}`,
    "--target=node",
    "--format=esm",
    "-e",
    "better-sqlite3",
  ],
  {
    cwd: appRoot,
    stdio: "inherit",
    shell: process.platform === "win32",
  },
);

if (build.status !== 0) {
  process.exit(build.status ?? 1);
}

const electronBin = path.join(
  appRoot,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "electron.cmd" : "electron",
);

const run = spawnSync(electronBin, [outFile, ...process.argv.slice(2)], {
  cwd: appRoot,
  env: {
    ...process.env,
    ELECTRON_RUN_AS_NODE: "1",
  },
  stdio: "inherit",
  shell: process.platform === "win32",
});

process.exit(run.status ?? 1);
