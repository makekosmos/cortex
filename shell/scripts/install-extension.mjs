#!/usr/bin/env node
// Устанавливает extension'а в user-installed location Kepler:
//
//   <APPDATA>/Kosmos/extensions/<id>/
//
// где <APPDATA> — platform-зависимая папка для roaming-данных пользователя
// (`%APPDATA%` на Windows, `~/Library/Application Support` на Mac,
// `~/.config` на Linux). <id> берётся из `manifest.json` исходной директории.
//
// Resolution chain в extension-host.ts ставит user-installed выше bundled —
// то есть свежая копия в %APPDATA% перекрывает версию, которая приехала с
// Kepler installer'ом. Удаление user-папки (см. uninstall-extension.mjs)
// откатывает обратно на bundled.
//
// Usage:
//   bun run --cwd shell ext:install <path-to-extension-dir>
//
// `<path-to-extension-dir>` должен быть готовый built extension: содержать
// `manifest.json`, `dist/` (для Vue) или `index.html` (для static), `icon.png`.
// Скрипт не запускает Vite build — это ответственность вызывающей стороны.

import { existsSync, readFileSync, cpSync, renameSync, rmSync, mkdirSync, statSync } from "node:fs";
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

function die(msg) {
  console.error(`[ext:install] ${msg}`);
  process.exit(1);
}

const source = process.argv[2];
if (!source) {
  die("usage: ext:install <path-to-extension-dir>");
}
const sourceAbs = path.resolve(source);
if (!existsSync(sourceAbs)) die(`source does not exist: ${sourceAbs}`);
const sourceStat = statSync(sourceAbs);
if (!sourceStat.isDirectory()) die(`source is not a directory: ${sourceAbs}`);

const manifestPath = path.join(sourceAbs, "manifest.json");
if (!existsSync(manifestPath)) {
  die(`no manifest.json in source: ${manifestPath}`);
}

let manifest;
try {
  manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
} catch (e) {
  die(`invalid manifest.json: ${e.message}`);
}
if (!manifest.id || typeof manifest.id !== "string") {
  die(`manifest.id missing or not a string`);
}

const root = resolveUserExtensionsRoot();
mkdirSync(root, { recursive: true });
const target = path.join(root, manifest.id);
const stamp = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
const tmp = `${target}.tmp-${stamp}`;
const old = existsSync(target) ? `${target}.old-${stamp}` : null;

console.log(`[ext:install] ${manifest.id} → ${target}`);

try {
  cpSync(sourceAbs, tmp, { recursive: true, errorOnExist: false });
  if (old) renameSync(target, old);
  renameSync(tmp, target);
  if (old) rmSync(old, { recursive: true, force: true });
  console.log(`[ext:install] ok`);
} catch (e) {
  console.error(`[ext:install] failed: ${e.message}`);
  // Best-effort rollback.
  try {
    if (old && !existsSync(target) && existsSync(old)) {
      renameSync(old, target);
    }
    rmSync(tmp, { recursive: true, force: true });
  } catch (rollbackErr) {
    console.error(`[ext:install] rollback also failed: ${rollbackErr.message}`);
  }
  process.exit(1);
}
