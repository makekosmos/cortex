import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const taskId = process.env.ARK_SMOKE_TASK_ID || "2026-04-26-ark-initial-plan-close";
const smokeRoot = path.join(repoRoot, ".agent", "tasks", taskId, "smoke");

fs.mkdirSync(smokeRoot, { recursive: true });

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
    args: ["test", "--manifest-path", "crates/ark-core/Cargo.toml"],
  },
  {
    name: "@makekosmos/ark published consumer",
    command: process.execPath,
    args: ["scripts/ark-consumer.mjs"],
  },
];

// Заметка: после Phase B-E + 6.0.A смок-матрица сжалась.
// — Eden, Dashboard, Delphi, Arrancador — Vue extensions в `extensions/<name>`,
//   у них нет отдельных Electron unit / e2e наборов (host берёт на себя через Kepler shell).
// — Standalone Eden удалён в Phase 6.0.A, миграции из vault'а Heart больше нет.
// — Standalone сценарии покрывает `bun run --cwd platform/desktop test:e2e`,
//   но он тяжёлый и должен запускаться явно, не как часть smoke.
//
// Если нужно добавить шаг — извлекай `cwd` через `path.join(repoRoot, ...)`,
// чтобы не зависеть от запуска из корня.

function runStep(step) {
  return new Promise((resolve, reject) => {
    console.log(`\n==> ${step.name}`);
    console.log(`$ ${step.command} ${step.args.join(" ")}`);

    // На Windows shell:true парсит командную строку через cmd, который
    // ломается на пробелах в path (`C:\Program Files\nodejs\node.exe`).
    // Quote'им команду явно — args останутся обычными.
    const command = isWindows ? `"${step.command}"` : step.command;
    const child = spawn(command, step.args, {
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
