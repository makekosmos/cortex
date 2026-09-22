#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from "node:fs";
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

// KOS-134: components/manager ships manager-gpui — a single-file Rust/GPUI
// exe staged under the packaged name resolvePackagedManagerExecutable()
// resolves in desktop/electron/manager-navigation.ts. The target triple is
// the toolchain.target the release BOM records for Windows builds.
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
copyFileSync(managerExe, path.join(managerStage, "Kosmos Manager.exe"));
for (const entry of readdirSync(managerRelease)) {
  if (entry.toLowerCase().endsWith(".dll"))
    copyFileSync(path.join(managerRelease, entry), path.join(managerStage, entry));
}

// KOS-137: components/agenda ships agenda-gpui — same single-file Rust/GPUI
// shape as Manager, staged under the packaged name
// resolvePackagedAgendaExecutable() resolves in desktop/electron/agenda-navigation.ts.
// agenda-gpui lives in its own repository: resolve the checkout from
// KOSMOS_AGENDA_GPUI_SRC or the ../agenda-gpui sibling (same convention the
// sibling extension repos use), then verify it sits on the commit
// desktop/component-pins.json records.
const agendaPins = JSON.parse(
  readFileSync(path.join(root, "desktop", "component-pins.json"), "utf8"),
);
const agendaPin = agendaPins.agenda_gpui;
if (!agendaPin?.commit || !/^[0-9a-f]{40}$/.test(agendaPin.commit)) {
  console.error(
    "[build-package-components] component-pins.json agenda_gpui.commit must be a 40-hex commit",
  );
  process.exit(1);
}
const agendaSrc = path.resolve(
  process.env.KOSMOS_AGENDA_GPUI_SRC?.trim() || path.join(root, "..", "agenda-gpui"),
);
if (!existsSync(path.join(agendaSrc, "Cargo.toml"))) {
  console.error(
    `[build-package-components] agenda-gpui checkout not found at ${agendaSrc} — ` +
      `clone ${agendaPin.repository} at ${agendaPin.commit} or set KOSMOS_AGENDA_GPUI_SRC`,
  );
  process.exit(1);
}
const agendaHead = spawnSync("git", ["rev-parse", "HEAD"], {
  cwd: agendaSrc,
  encoding: "utf8",
  windowsHide: true,
});
const agendaCommit = agendaHead.stdout?.trim();
if (agendaHead.status !== 0 || agendaCommit !== agendaPin.commit) {
  console.error(
    `[build-package-components] agenda-gpui at ${agendaSrc} is on ${agendaCommit ?? "unreadable HEAD"}, ` +
      `component-pins.json requires ${agendaPin.commit}`,
  );
  process.exit(1);
}
const agendaRelease = path.join(agendaSrc, "target", MANAGER_TARGET, "release");
const agendaBuild = spawnSync(
  "cargo",
  [
    "build",
    "--locked",
    "--release",
    "--target",
    MANAGER_TARGET,
    "--manifest-path",
    path.join(agendaSrc, "Cargo.toml"),
    "--target-dir",
    path.join(agendaSrc, "target"),
  ],
  {
    cwd: agendaSrc,
    stdio: "inherit",
    windowsHide: true,
    // VERSIONINFO inside Kosmos Agenda.exe carries the desktop release
    // version (agenda-gpui build.rs falls back to its own crate version).
    env: { ...process.env, KOSMOS_AGENDA_VERSION: version },
  },
);
if (agendaBuild.status !== 0) process.exit(agendaBuild.status ?? 1);
const agendaExe = path.join(agendaRelease, "agenda-gpui.exe");
if (!existsSync(agendaExe)) {
  console.error(`[build-package-components] missing ${agendaExe}`);
  process.exit(1);
}
const agendaStage = path.join(root, "desktop", ".tmp", "components", "agenda", "win-unpacked");
rmSync(agendaStage, { recursive: true, force: true });
mkdirSync(agendaStage, { recursive: true });
copyFileSync(agendaExe, path.join(agendaStage, "Kosmos Agenda.exe"));
for (const entry of readdirSync(agendaRelease)) {
  if (entry.toLowerCase().endsWith(".dll"))
    copyFileSync(path.join(agendaRelease, entry), path.join(agendaStage, entry));
}

// components/host stays the Electron Package Host (docs/gpui-host-decision.md).
for (const component of ["host"]) {
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
