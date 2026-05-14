#!/usr/bin/env node
// Удаляет user-installed extension'а из <APPDATA>/Kosmos/extensions/<id>/.
// После этого resolution chain поднимет bundled версию (если есть в
// Kepler installer'е) или extension просто пропадёт из listExtensions.
//
// Bundled extension'ы НЕ затрагиваются — они read-only внутри install
// директории Kepler.
//
// Usage:
//   bun run --cwd apps/kepler-shell ext:uninstall <id>

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

const id = process.argv[2];
if (!id) {
  console.error("[ext:uninstall] usage: ext:uninstall <id>");
  process.exit(1);
}

const target = path.join(resolveUserExtensionsRoot(), id);
if (!existsSync(target)) {
  console.log(`[ext:uninstall] ${id}: nothing to remove (${target} doesn't exist)`);
  process.exit(0);
}

try {
  rmSync(target, { recursive: true, force: true });
  console.log(`[ext:uninstall] ${id}: removed ${target}`);
} catch (e) {
  console.error(`[ext:uninstall] failed: ${e.message}`);
  process.exit(1);
}
