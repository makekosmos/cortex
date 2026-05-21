#!/usr/bin/env node
// Устанавливает extension в user-installed location Kepler:
//
//   <APPDATA>/Kosmos/extensions/<id>/
//
// Принимает:
//   - путь к директории с готовым manifest.json (dev flow);
//   - путь к `.kext` или `.zip` архиву (production flow).
//
// Перед install:
//   - validate manifest (id, name);
//   - keplerApiVersion compat check против KEPLER_API_VERSION из shell;
//   - preview печатается в stdout.
//
// Backup: текущая `<APPDATA>/Kosmos/extensions/<id>/` (если есть) копируется
// в `extensions-backups/<id>/<ISO-timestamp>/` перед extract'ом.
// Сохраняется до 5 backup'ов на id, старые удаляются.
//
// Atomic write: extract в `extensions-tmp/<id>-<stamp>/`, validate, rename
// → `extensions/<id>/`.
//
// Usage:
//   bun run --cwd shell ext:install <path-to-dir-or-kext>
//   bun run --cwd shell ext:install ./extension.kext

import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import os from "node:os";
import { readZip, extractZip, safeEntryName } from "./zip-utils.mjs";

// Должно совпадать с shell/electron/kepler-api.ts. При bump'е обновлять оба.
const KEPLER_API_VERSION = "1.0.0";
const MAX_BACKUPS = 5;

function resolveDataDir() {
  // KOSMOS_DATA_DIR — для тестов / dev override.
  if (process.env.KOSMOS_DATA_DIR) return process.env.KOSMOS_DATA_DIR;
  const platform = process.platform;
  let base;
  if (platform === "win32") {
    base = process.env.APPDATA ?? path.join(os.homedir(), "AppData", "Roaming");
  } else if (platform === "darwin") {
    base = path.join(os.homedir(), "Library", "Application Support");
  } else {
    base = process.env.XDG_CONFIG_HOME ?? path.join(os.homedir(), ".config");
  }
  return path.join(base, "Kosmos");
}

function die(msg) {
  console.error(`[ext:install] ${msg}`);
  process.exit(1);
}

// Минимальный semver matcher — копия из shell/electron/kepler-api.ts.
// Дублируем (40 строк), чтобы скрипт не зависел от dist-electron bundle'а.
function parseSemver(v) {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(String(v).trim());
  if (!m) return null;
  return { major: +m[1], minor: +m[2], patch: +m[3] };
}
function cmp(a, b) {
  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  return a.patch - b.patch;
}
function satisfiesSemver(version, range) {
  const v = parseSemver(version);
  if (!v) return false;
  const branches = String(range)
    .split("||")
    .map((s) => s.trim())
    .filter(Boolean);
  return branches.some((b) => matchSingle(b, v));
}
function matchSingle(range, v) {
  const r = range.trim();
  if (r === "" || r === "*") return true;
  if (r.startsWith("^")) {
    const b = parseSemver(r.slice(1));
    if (!b || cmp(v, b) < 0) return false;
    if (b.major === 0) {
      if (b.minor === 0) return v.major === 0 && v.minor === 0 && v.patch === b.patch;
      return v.major === 0 && v.minor === b.minor;
    }
    return v.major === b.major;
  }
  if (r.startsWith("~")) {
    const b = parseSemver(r.slice(1));
    if (!b || cmp(v, b) < 0) return false;
    return v.major === b.major && v.minor === b.minor;
  }
  const cm = /^(>=|<=|>|<|=)\s*(.+)$/.exec(r);
  if (cm) {
    const b = parseSemver(cm[2]);
    if (!b) return false;
    const c = cmp(v, b);
    return (
      (cm[1] === ">=" && c >= 0) ||
      (cm[1] === "<=" && c <= 0) ||
      (cm[1] === ">" && c > 0) ||
      (cm[1] === "<" && c < 0) ||
      (cm[1] === "=" && c === 0)
    );
  }
  const exact = parseSemver(r);
  if (exact) return cmp(v, exact) === 0;
  return false;
}

function copyDirSync(src, dst) {
  mkdirSync(dst, { recursive: true });
  for (const e of readdirSync(src, { withFileTypes: true })) {
    const s = path.join(src, e.name);
    const d = path.join(dst, e.name);
    if (e.isDirectory()) copyDirSync(s, d);
    else if (e.isFile()) writeFileSync(d, readFileSync(s));
  }
}

const source = process.argv[2];
if (!source) {
  die("usage: ext:install <path-to-dir-or-kext>");
}
const sourceAbs = path.resolve(source);
if (!existsSync(sourceAbs)) die(`source does not exist: ${sourceAbs}`);
const sourceStat = statSync(sourceAbs);

let manifest;
let isZipSource = false;
if (sourceStat.isDirectory()) {
  const mp = path.join(sourceAbs, "manifest.json");
  if (!existsSync(mp)) die(`no manifest.json in source: ${mp}`);
  try {
    manifest = JSON.parse(readFileSync(mp, "utf8"));
  } catch (e) {
    die(`invalid manifest.json: ${e.message}`);
  }
} else if (sourceAbs.toLowerCase().endsWith(".kext") || sourceAbs.toLowerCase().endsWith(".zip")) {
  isZipSource = true;
  let entries;
  try {
    entries = readZip(sourceAbs);
  } catch (e) {
    die(`cannot read .kext: ${e.message}`);
  }
  const mEntry = entries.find((e) => e.name === "manifest.json" && !e.isDir);
  if (!mEntry) die(`no manifest.json в .kext (root)`);
  try {
    manifest = JSON.parse(mEntry.data.toString("utf8"));
  } catch (e) {
    die(`invalid manifest.json в .kext: ${e.message}`);
  }
  // Path traversal sanity на все entry names.
  for (const e of entries) {
    try {
      safeEntryName(e.name);
    } catch (err) {
      die(`unsafe entry in .kext: ${err.message}`);
    }
  }
} else {
  die(`unsupported source (need directory or .kext/.zip): ${sourceAbs}`);
}

if (!manifest.id || typeof manifest.id !== "string") {
  die(`manifest.id missing or not a string`);
}
if (!/^[\w][\w.-]*$/.test(manifest.id)) {
  die(`manifest.id невалиден: ${manifest.id}`);
}
if (!manifest.name) {
  die(`manifest.name missing`);
}

console.log(`[ext:install] preview:`);
console.log(`  id:          ${manifest.id}`);
console.log(`  name:        ${manifest.name}`);
console.log(`  version:     ${manifest.version ?? "(not set)"}`);
console.log(`  description: ${manifest.description ?? "(not set)"}`);
console.log(`  author:      ${manifest.author ?? "(not set)"}`);
console.log(`  api req:     ${manifest.keplerApiVersion ?? "(legacy)"}`);
console.log(
  `  permissions: ${Array.isArray(manifest.permissions) ? manifest.permissions.join(", ") : "(none)"}`,
);

if (manifest.keplerApiVersion) {
  if (!satisfiesSemver(KEPLER_API_VERSION, manifest.keplerApiVersion)) {
    die(
      `keplerApiVersion mismatch: extension требует ${manifest.keplerApiVersion}, ` +
        `установлено ${KEPLER_API_VERSION}`,
    );
  }
}

const dataDir = resolveDataDir();
const root = path.join(dataDir, "extensions");
const backupsRoot = path.join(dataDir, "extensions-backups");
const tmpRoot = path.join(dataDir, "extensions-tmp");
mkdirSync(root, { recursive: true });
mkdirSync(tmpRoot, { recursive: true });

const target = path.join(root, manifest.id);
const stamp = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
const tmp = path.join(tmpRoot, `${manifest.id}-${stamp}`);

console.log(`[ext:install] ${manifest.id} → ${target}`);

// Backup current если есть.
let backupPath = null;
if (existsSync(target)) {
  const idBackupsRoot = path.join(backupsRoot, manifest.id);
  mkdirSync(idBackupsRoot, { recursive: true });
  const isoStamp = new Date().toISOString().replace(/[:.]/g, "-");
  backupPath = path.join(idBackupsRoot, isoStamp);
  try {
    copyDirSync(target, backupPath);
    console.log(`[ext:install] backup → ${backupPath}`);
  } catch (e) {
    die(`backup failed: ${e.message}`);
  }
  // Prune старые backup'ы.
  try {
    const list = readdirSync(idBackupsRoot)
      .filter((n) => {
        try {
          return statSync(path.join(idBackupsRoot, n)).isDirectory();
        } catch {
          return false;
        }
      })
      .sort();
    while (list.length > MAX_BACKUPS) {
      const old = list.shift();
      rmSync(path.join(idBackupsRoot, old), { recursive: true, force: true });
    }
  } catch (e) {
    console.warn(`[ext:install] backup prune warning: ${e.message}`);
  }
}

try {
  if (isZipSource) {
    extractZip(sourceAbs, tmp);
  } else {
    copyDirSync(sourceAbs, tmp);
  }
  // Validate manifest from extracted tmp.
  const tmpManifestPath = path.join(tmp, "manifest.json");
  if (!existsSync(tmpManifestPath)) {
    throw new Error(`manifest.json missing after extract`);
  }
  const extractedManifest = JSON.parse(readFileSync(tmpManifestPath, "utf8"));
  if (extractedManifest.id !== manifest.id) {
    throw new Error(`manifest.id mismatch after extract`);
  }

  let oldDir = null;
  if (existsSync(target)) {
    oldDir = `${target}.old-${stamp}`;
    renameSync(target, oldDir);
  }
  renameSync(tmp, target);
  if (oldDir) rmSync(oldDir, { recursive: true, force: true });
  console.log(`[ext:install] ok`);
} catch (e) {
  console.error(`[ext:install] failed: ${e.message}`);
  // Best-effort: вернуть backup → target.
  try {
    if (backupPath && !existsSync(target) && existsSync(backupPath)) {
      copyDirSync(backupPath, target);
      console.error(`[ext:install] restored from backup`);
    }
    rmSync(tmp, { recursive: true, force: true });
  } catch (rollbackErr) {
    console.error(`[ext:install] rollback failed: ${rollbackErr.message}`);
  }
  process.exit(1);
}
