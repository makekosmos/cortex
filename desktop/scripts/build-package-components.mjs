#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { env } from "./brand.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
// Release preflight demands a clean worktree and a BOM; local staging (for
// `build:desktop -- --local`) skips it — the pin checks below still apply.
if (env("RELEASE_LOCAL") !== "1") {
  const preflight = spawnSync(
    process.execPath,
    [path.join(root, "desktop", "scripts", "release-preflight.mjs"), "--platform", "win"],
    { cwd: path.join(root, "desktop"), stdio: "inherit", windowsHide: true },
  );
  if (preflight.status !== 0) process.exit(preflight.status ?? 1);
}
const icons = spawnSync(
  process.execPath,
  [path.join(root, "desktop", "scripts/build-app-icons.mjs")],
  { cwd: path.join(root, "desktop"), stdio: "inherit", windowsHide: true },
);
if (icons.status !== 0) process.exit(icons.status ?? 1);

// KOS-134: components/manager ships manager-gpui — a single-file Rust/GPUI
// exe staged under the packaged name the Engine tray resolves. The target
// triple is the toolchain.target the release BOM records for Windows builds.
// KOS-265: Agenda/Memoria/Dictation are no longer bundled — the Engine
// installs them as native apps from GitHub Releases.
const MANAGER_TARGET = "x86_64-pc-windows-msvc";
const managerRelease = path.join(root, "manager-gpui", "target", MANAGER_TARGET, "release");
const managerBuild = spawnSync(
  "cargo",
  [
    "build",
    "--locked",
    "--release",
    "--target",
    MANAGER_TARGET,
    "--manifest-path",
    path.join(root, "manager-gpui", "Cargo.toml"),
    "--target-dir",
    path.join(root, "manager-gpui", "target"),
  ],
  { cwd: path.join(root, "manager-gpui"), stdio: "inherit", windowsHide: true },
);
if (managerBuild.status !== 0) process.exit(managerBuild.status ?? 1);
const managerExe = path.join(managerRelease, "manager-gpui.exe");
if (!existsSync(managerExe)) {
  console.error(`[build-package-components] missing ${managerExe}`);
  process.exit(1);
}
const managerStage = path.join(root, "desktop", ".tmp", "components", "manager", "win-unpacked");
rmSync(managerStage, { recursive: true, force: true });
mkdirSync(managerStage, { recursive: true });
copyFileSync(managerExe, path.join(managerStage, "Mundus Manager.exe"));
for (const entry of readdirSync(managerRelease)) {
  if (entry.toLowerCase().endsWith(".dll"))
    copyFileSync(path.join(managerRelease, entry), path.join(managerStage, entry));
}
