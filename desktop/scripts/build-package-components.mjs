#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
// Release preflight demands a clean worktree and a BOM; local staging (for
// `build:desktop -- --local`) skips it — the pin checks below still apply.
if (process.env.KOSMOS_RELEASE_LOCAL !== "1") {
  const preflight = spawnSync(
    process.execPath,
    [path.join(root, "desktop", "scripts", "release-preflight.mjs"), "--platform", "win"],
    { cwd: path.join(root, "desktop"), stdio: "inherit", windowsHide: true },
  );
  if (preflight.status !== 0) process.exit(preflight.status ?? 1);
}
const version = JSON.parse(
  readFileSync(path.join(root, "desktop", "release-versions.json"), "utf8"),
).win;
const icons = spawnSync(
  process.execPath,
  [path.join(root, "desktop", "scripts/build-app-icons.mjs")],
  { cwd: path.join(root, "desktop"), stdio: "inherit", windowsHide: true },
);
if (icons.status !== 0) process.exit(icons.status ?? 1);

// KOS-134: components/manager ships manager-gpui — a single-file Rust/GPUI
// exe staged under the packaged name the Engine tray resolves. The target
// triple is the toolchain.target the release BOM records for Windows builds.
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
// shape as Manager, staged under the packaged name the Engine tray resolves.
// agenda-gpui lives in its own repository: resolve the checkout from
// KOSMOS_AGENDA_GPUI_SRC or the ../agenda-gpui sibling (same convention the
// sibling extension repos use), then verify it sits on the commit
// desktop/component-pins.json records.
const componentPins = JSON.parse(
  readFileSync(path.join(root, "desktop", "component-pins.json"), "utf8"),
);
const agendaPin = componentPins.agenda_gpui;
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

// KOS-156: components/memoria ships memoria-gpui — same single-file
// Rust/GPUI shape as Agenda, staged under the packaged name the Engine tray
// resolves. memoria-gpui lives in its own
// repository: resolve the checkout from KOSMOS_MEMORIA_GPUI_SRC or the
// ../memoria-gpui sibling, then verify it sits on the commit
// desktop/component-pins.json records.
const memoriaPin = componentPins.memoria_gpui;
if (!memoriaPin?.commit || !/^[0-9a-f]{40}$/.test(memoriaPin.commit)) {
  console.error(
    "[build-package-components] component-pins.json memoria_gpui.commit must be a 40-hex commit",
  );
  process.exit(1);
}
const memoriaSrc = path.resolve(
  process.env.KOSMOS_MEMORIA_GPUI_SRC?.trim() || path.join(root, "..", "memoria-gpui"),
);
if (!existsSync(path.join(memoriaSrc, "Cargo.toml"))) {
  console.error(
    `[build-package-components] memoria-gpui checkout not found at ${memoriaSrc} — ` +
      `clone ${memoriaPin.repository} at ${memoriaPin.commit} or set KOSMOS_MEMORIA_GPUI_SRC`,
  );
  process.exit(1);
}
const memoriaHead = spawnSync("git", ["rev-parse", "HEAD"], {
  cwd: memoriaSrc,
  encoding: "utf8",
  windowsHide: true,
});
const memoriaCommit = memoriaHead.stdout?.trim();
if (memoriaHead.status !== 0 || memoriaCommit !== memoriaPin.commit) {
  console.error(
    `[build-package-components] memoria-gpui at ${memoriaSrc} is on ${memoriaCommit ?? "unreadable HEAD"}, ` +
      `component-pins.json requires ${memoriaPin.commit}`,
  );
  process.exit(1);
}
const memoriaRelease = path.join(memoriaSrc, "target", MANAGER_TARGET, "release");
const memoriaBuild = spawnSync(
  "cargo",
  [
    "build",
    "--locked",
    "--release",
    "--target",
    MANAGER_TARGET,
    "--manifest-path",
    path.join(memoriaSrc, "Cargo.toml"),
    "--target-dir",
    path.join(memoriaSrc, "target"),
  ],
  {
    cwd: memoriaSrc,
    stdio: "inherit",
    windowsHide: true,
    // VERSIONINFO inside Kosmos Memoria.exe carries the desktop release
    // version (memoria-gpui build.rs falls back to its own crate version).
    env: { ...process.env, KOSMOS_MEMORIA_VERSION: version },
  },
);
if (memoriaBuild.status !== 0) process.exit(memoriaBuild.status ?? 1);
const memoriaExe = path.join(memoriaRelease, "memoria-gpui.exe");
if (!existsSync(memoriaExe)) {
  console.error(`[build-package-components] missing ${memoriaExe}`);
  process.exit(1);
}
const memoriaStage = path.join(root, "desktop", ".tmp", "components", "memoria", "win-unpacked");
rmSync(memoriaStage, { recursive: true, force: true });
mkdirSync(memoriaStage, { recursive: true });
copyFileSync(memoriaExe, path.join(memoriaStage, "Kosmos Memoria.exe"));
for (const entry of readdirSync(memoriaRelease)) {
  if (entry.toLowerCase().endsWith(".dll"))
    copyFileSync(path.join(memoriaRelease, entry), path.join(memoriaStage, entry));
}

// KOS-241: components/dictation ships dictation-gpui — same single-file
// Rust/GPUI shape as the other components. The root crate is named
// `dictation-gpui` inside the makekosmos/dictation repository.
const dictationPin = componentPins.dictation_gpui;
if (!dictationPin?.commit || !/^[0-9a-f]{40}$/.test(dictationPin.commit)) {
  console.error(
    "[build-package-components] component-pins.json dictation_gpui.commit must be a 40-hex commit",
  );
  process.exit(1);
}
const dictationSrc = path.resolve(
  process.env.KOSMOS_DICTATION_GPUI_SRC?.trim() || path.join(root, "..", "dictation"),
);
if (!existsSync(path.join(dictationSrc, "Cargo.toml"))) {
  console.error(
    `[build-package-components] dictation-gpui checkout not found at ${dictationSrc} — ` +
      `clone ${dictationPin.repository} at ${dictationPin.commit} or set KOSMOS_DICTATION_GPUI_SRC`,
  );
  process.exit(1);
}
const dictationHead = spawnSync("git", ["rev-parse", "HEAD"], {
  cwd: dictationSrc,
  encoding: "utf8",
  windowsHide: true,
});
const dictationCommit = dictationHead.stdout?.trim();
if (dictationHead.status !== 0 || dictationCommit !== dictationPin.commit) {
  console.error(
    `[build-package-components] dictation-gpui at ${dictationSrc} is on ${dictationCommit ?? "unreadable HEAD"}, ` +
      `component-pins.json requires ${dictationPin.commit}`,
  );
  process.exit(1);
}
const dictationRelease = path.join(dictationSrc, "target", MANAGER_TARGET, "release");
const dictationBuild = spawnSync(
  "cargo",
  [
    "build",
    "--locked",
    "--release",
    "--target",
    MANAGER_TARGET,
    "--manifest-path",
    path.join(dictationSrc, "Cargo.toml"),
    "--target-dir",
    path.join(dictationSrc, "target"),
  ],
  {
    cwd: dictationSrc,
    stdio: "inherit",
    windowsHide: true,
    env: { ...process.env, KOSMOS_DICTATION_VERSION: version },
  },
);
if (dictationBuild.status !== 0) process.exit(dictationBuild.status ?? 1);
const dictationExe = path.join(dictationRelease, "dictation-gpui.exe");
if (!existsSync(dictationExe)) {
  console.error(`[build-package-components] missing ${dictationExe}`);
  process.exit(1);
}
const dictationStage = path.join(
  root,
  "desktop",
  ".tmp",
  "components",
  "dictation",
  "win-unpacked",
);
rmSync(dictationStage, { recursive: true, force: true });
mkdirSync(dictationStage, { recursive: true });
copyFileSync(dictationExe, path.join(dictationStage, "Kosmos Dictation.exe"));
for (const entry of readdirSync(dictationRelease)) {
  if (entry.toLowerCase().endsWith(".dll"))
    copyFileSync(path.join(dictationRelease, entry), path.join(dictationStage, entry));
}
