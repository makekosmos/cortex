import { spawn } from "node:child_process";
import process from "node:process";

async function runBunStep(step: string) {
  await new Promise<void>((resolve, reject) => {
    const child = spawn(
      "powershell",
      ["-NoProfile", "-Command", `bun run ${step}`],
      {
      cwd: process.cwd(),
      env: process.env,
      stdio: "inherit",
      },
    );

    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(new Error(`${step} exited with code ${String(code)}`));
    });
  });
}

async function main() {
  await runBunStep("predev");
  await runBunStep("build:renderer");
  await runBunStep("build:main");
  await runBunStep("build:preload");
}

if (import.meta.main) {
  void main().catch((error) => {
    process.stderr.write(
      `[build] ${error instanceof Error ? error.stack ?? error.message : String(error)}\n`,
    );
    process.exit(1);
  });
}
