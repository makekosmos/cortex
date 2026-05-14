import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const taskId = process.env.ARK_SMOKE_TASK_ID || "2026-04-26-ark-initial-plan-close";
const smokeRoot = path.join(repoRoot, ".agent", "tasks", taskId, "smoke");
const dashboardDb = path.join(smokeRoot, "dashboard", "smoke-dashboard.db");

fs.mkdirSync(path.dirname(dashboardDb), { recursive: true });

const isWindows = process.platform === "win32";
const steps = [
  {
    name: "Guard ARK app write boundaries",
    command: process.execPath,
    args: ["scripts/check-ark-write-boundaries.mjs"],
  },
  {
    name: "ARK core Rust tests",
    command: "cargo",
    args: ["test", "--manifest-path", "crates/ark-core/rust/Cargo.toml"],
  },
  {
    name: "usage-tracker Rust tests",
    command: "cargo",
    args: ["test", "--manifest-path", "services/usage-tracker/Cargo.toml"],
  },
  {
    name: "@kepler/ark typecheck",
    command: "bun",
    args: ["run", "--cwd", "packages/ark", "typecheck"],
  },
  {
    name: "Arrancador unit tests",
    command: "bun",
    args: ["run", "--cwd", "apps/arrancador", "test"],
  },
  {
    name: "Arrancador typecheck",
    command: "bun",
    args: ["run", "--cwd", "apps/arrancador", "typecheck"],
  },
  {
    name: "Eden ARK migration test",
    command: "bun",
    args: ["run", "--cwd", "apps/eden/ts", "test:ark-migration"],
  },
  {
    name: "Eden build",
    command: "bun",
    args: ["run", "build"],
    cwd: path.join(repoRoot, "apps", "eden", "ts"),
  },
  {
    name: "Eden typed-note e2e",
    command: "bunx",
    args: [
      "playwright",
      "test",
      "tests/app.spec.ts",
      "--config",
      "playwright.config.ts",
      "--grep",
      "custom note type",
    ],
    cwd: path.join(repoRoot, "apps", "eden", "ts"),
  },
  {
    name: "Dashboard smoke seed",
    command: process.execPath,
    args: [
      "--experimental-strip-types",
      "apps/dashboard/scripts/seedSmokeDb.ts",
      "--db-path",
      dashboardDb,
    ],
  },
  {
    name: "Dashboard smoke analytics",
    command: process.execPath,
    args: [
      "--experimental-strip-types",
      "apps/dashboard/scripts/smokeAnalytics.ts",
      "--db-path",
      dashboardDb,
    ],
  },
];

function runStep(step) {
  return new Promise((resolve, reject) => {
    console.log(`\n==> ${step.name}`);
    console.log(`$ ${step.command} ${step.args.join(" ")}`);

    const child = spawn(step.command, step.args, {
      cwd: step.cwd ?? repoRoot,
      env: {
        ...process.env,
        ARK_SMOKE_ROOT: smokeRoot,
      },
      stdio: "inherit",
      shell: isWindows,
    });

    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${step.name} failed with exit code ${code}`));
      }
    });
  });
}

for (const step of steps) {
  await runStep(step);
}

console.log(`\nARK smoke matrix passed. Smoke root: ${smokeRoot}`);
