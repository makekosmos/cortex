#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const preflight = spawnSync(
  process.execPath,
  [path.join(root, "desktop", "scripts", "release-preflight.mjs"), "--platform", "win"],
  { cwd: path.join(root, "desktop"), stdio: "inherit", windowsHide: true },
);
if (preflight.status !== 0) process.exit(preflight.status ?? 1);
const version = JSON.parse(
  readFileSync(path.join(root, "desktop", "release-versions.json"), "utf8"),
).win;
const pnpm = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
const shell = pnpm.endsWith(".cmd");
const shellArg = (value) => (shell ? `"${value}"` : value);
const icons = spawnSync(
  process.execPath,
  [path.join(root, "desktop", "scripts/build-app-icons.mjs")],
  { cwd: path.join(root, "desktop"), stdio: "inherit", windowsHide: true },
);
if (icons.status !== 0) process.exit(icons.status ?? 1);
const components = ["manager", "host"];
for (const component of components) {
  const cwd = path.join(root, component);
  const build = spawnSync(pnpm, ["run", "build"], {
    cwd,
    stdio: "inherit",
    windowsHide: true,
    shell,
  });
  if (build.status !== 0) process.exit(build.status ?? 1);
  const output = path.join(root, "desktop", ".tmp", "components", component);
  const packaged = spawnSync(
    pnpm,
    [
      "exec",
      "electron-builder",
      "--win",
      "--dir",
      "--publish",
      "never",
      `--config.extraMetadata.version=${version}`,
      "--config.win.signExecutable=false",
      `--config.directories.output=${shellArg(output)}`,
    ],
    { cwd, stdio: "inherit", windowsHide: true, shell },
  );
  if (packaged.status !== 0) process.exit(packaged.status ?? 1);
}
