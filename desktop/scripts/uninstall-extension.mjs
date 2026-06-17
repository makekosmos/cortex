#!/usr/bin/env node
// Удаляет user-installed extension'а из <APPDATA>/Kosmos/extensions/<id>/.
// После этого resolution chain поднимет bundled версию (если есть в
// Kepler installer'е) или extension просто пропадёт из listExtensions.
//
// Bundled extension'ы НЕ затрагиваются — они read-only внутри install
// директории Kepler.
//
// User data в <APPDATA>/Kosmos/extensions-data/<id>/ по умолчанию
// сохраняется, чтобы reinstall не терял settings / window-state. Чтобы
// удалить и user data — передай флаг --purge-data.
//
// Usage:
//   bun run --cwd platform/desktop ext:uninstall <id> [--purge-data]

import { existsSync, rmSync } from "node:fs";
import path from "node:path";
import os from "node:os";

function resolveUserExtensionsRoot() {
  const platform = process.platform;
  let base;
  if (platform === "win32") {
    base = process.env.APPDATA ?? path.join(os.homedir(), "AppData", "Roaming");
  } else if (platform === "darwin") {
    base = path.join(os.homedir(), "Library", "Application Support");
  } else {
    base = process.env.XDG_CONFIG_HOME ?? path.join(os.homedir(), ".config");
  }
  return path.join(base, "Kosmos", "extensions");
}

function resolveUserExtensionsDataRoot() {
  const platform = process.platform;
  let base;
  if (platform === "win32") {
    base = process.env.APPDATA ?? path.join(os.homedir(), "AppData", "Roaming");
  } else if (platform === "darwin") {
    base = path.join(os.homedir(), "Library", "Application Support");
  } else {
    base = process.env.XDG_CONFIG_HOME ?? path.join(os.homedir(), ".config");
  }
  return path.join(base, "Kosmos", "extensions-data");
}

const args = process.argv.slice(2);
const purgeData = args.includes("--purge-data");
const id = args.find((a) => !a.startsWith("--"));

if (!id) {
  console.error("[ext:uninstall] usage: ext:uninstall <id> [--purge-data]");
  process.exit(1);
}

const codeTarget = path.join(resolveUserExtensionsRoot(), id);
const dataTarget = path.join(resolveUserExtensionsDataRoot(), id);

if (!existsSync(codeTarget)) {
  console.log(`[ext:uninstall] ${id}: code dir not found (${codeTarget})`);
} else {
  try {
    rmSync(codeTarget, { recursive: true, force: true });
    console.log(`[ext:uninstall] ${id}: removed code at ${codeTarget}`);
  } catch (e) {
    console.error(`[ext:uninstall] failed to remove code: ${e.message}`);
    process.exit(1);
  }
}

if (purgeData) {
  if (existsSync(dataTarget)) {
    try {
      rmSync(dataTarget, { recursive: true, force: true });
      console.log(`[ext:uninstall] ${id}: removed user data at ${dataTarget}`);
    } catch (e) {
      console.error(`[ext:uninstall] failed to remove user data: ${e.message}`);
      process.exit(1);
    }
  } else {
    console.log(`[ext:uninstall] ${id}: no user data to remove (${dataTarget})`);
  }
} else if (existsSync(dataTarget)) {
  console.log(
    `[ext:uninstall] ${id}: user data preserved at ${dataTarget} (pass --purge-data to also remove)`,
  );
}
