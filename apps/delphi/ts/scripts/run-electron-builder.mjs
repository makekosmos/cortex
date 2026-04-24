import { spawnSync } from "node:child_process";

const platformTargets = {
  win32: ["--win"],
  darwin: ["--mac"],
  linux: ["--linux"],
};

const targets = platformTargets[process.platform];

if (!targets) {
  console.error(`Unsupported platform for Delphi packaging: ${process.platform}`);
  process.exit(1);
}

const result = spawnSync("electron-builder", targets, {
  stdio: "inherit",
  shell: true,
});

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}
