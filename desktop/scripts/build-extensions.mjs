#!/usr/bin/env node
// Builds all repo extensions:
// - Vue extensions through the shared Vite extension config.
// - Native extensions through Cargo release builds, using `native.devExecutable` for dev-tree runs.

import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { findRepoExtensionEntry, listRepoExtensionEntries } from "./repo-extension-roots.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const shellRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(shellRoot, "..", "..");
const viteBin = path.join(shellRoot, "node_modules", "vite", "bin", "vite.js");

function readManifest(id) {
  return findRepoExtensionEntry(repoRoot, id)?.manifest ?? null;
}

function extensionIds() {
  return listRepoExtensionEntries(repoRoot).map((entry) => entry.id);
}

function parseCsv(value) {
  const ids = value
    .split(",")
    .map((id) => id.trim())
    .filter(Boolean);
  return Array.from(new Set(ids));
}

function argValue(name) {
  const index = process.argv.indexOf(name);
  if (index >= 0 && process.argv[index + 1] && !process.argv[index + 1].startsWith("--")) {
    return process.argv[index + 1];
  }
  const prefix = `${name}=`;
  const match = process.argv.find((arg) => arg.startsWith(prefix));
  return match ? match.slice(prefix.length) : null;
}

function hasArg(name) {
  return process.argv.includes(name) || process.argv.some((arg) => arg.startsWith(`${name}=`));
}

function changedExtensionIds(allIds) {
  let output = "";
  try {
    output = execFileSync("git", ["diff", "--name-only", "HEAD", "--"], {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
  } catch {
    output = execFileSync("git", ["diff", "--name-only", "--"], {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
  }
  try {
    const untracked = execFileSync("git", ["ls-files", "--others", "--exclude-standard"], {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    output += `\n${untracked}`;
  } catch {
    // `--changed` remains useful outside a git checkout; tracked diff above is enough.
  }

  const files = output
    .split(/\r?\n/)
    .map((line) => line.trim().replaceAll("\\", "/"))
    .filter(Boolean);

  if (
    files.some(
      (file) =>
        file === "pnpm-lock.yaml" ||
        file === "cortex/desktop/package.json" ||
        file === "cortex/desktop/vite.extensions.config.mjs" ||
        file === "cortex/desktop/scripts/build-extensions.mjs" ||
        file.startsWith("imago/") ||
        file.startsWith("arca-sdk/") ||
        file.startsWith("core/"),
    )
  ) {
    return allIds;
  }

  const changed = new Set();
  for (const file of files) {
    const entry = listRepoExtensionEntries(repoRoot).find((candidate) =>
      file.startsWith(`${candidate.rootName}/${candidate.folder}/`),
    );
    if (entry && allIds.includes(entry.id)) changed.add(entry.id);
  }
  return Array.from(changed);
}

if (hasArg("--help") || hasArg("-h")) {
  console.log(`Usage:
  node scripts/build-extensions.mjs
  node scripts/build-extensions.mjs --only <id[,id...]>
  node scripts/build-extensions.mjs --only=<id[,id...]>
  node scripts/build-extensions.mjs --changed

Options:
  --only <csv>     Build selected extension ids.
  --changed        Build extensions affected by git changes.
  --vue-only       Build only Vue extensions.
  --skip-native    Alias for --vue-only in fast workflows.
`);
  process.exit(0);
}

const allIds = extensionIds();
const onlyValue = argValue("--only");
const onlyIds = onlyValue !== null ? parseCsv(onlyValue) : [];
const changedMode = hasArg("--changed");

if (changedMode && onlyValue !== null) {
  throw new Error("[build:extensions] --only and --changed are mutually exclusive");
}
if (onlyValue !== null && onlyIds.length === 0) {
  throw new Error("[build:extensions] --only requires at least one extension id");
}

const ids = changedMode ? changedExtensionIds(allIds) : onlyIds.length > 0 ? onlyIds : allIds;

for (const id of ids) {
  if (!allIds.includes(id)) {
    throw new Error(
      `[build:extensions] unknown extension '${id}'. Valid ids: ${allIds.join(", ")}`,
    );
  }
}

const vueOnly = hasArg("--vue-only");
const skipNative = hasArg("--skip-native");
const vueIds = ids.filter((id) => readManifest(id)?.kind === "vue");
const nativeIds =
  vueOnly || skipNative ? [] : ids.filter((id) => readManifest(id)?.kind === "native");

if (ids.length === 0) {
  console.log("[build:extensions] no matching extensions to build");
  process.exit(0);
}
if (onlyValue !== null && vueIds.length === 0 && nativeIds.length === 0) {
  throw new Error("[build:extensions] selected extensions have no buildable targets");
}

for (const id of vueIds) {
  console.log("[build:extensions]", id);
  execFileSync(
    process.execPath,
    [
      viteBin,
      "build",
      "--configLoader",
      "native",
      "--config",
      "vite.extensions.config.mjs",
      "--mode",
      id,
    ],
    {
      cwd: shellRoot,
      stdio: "inherit",
    },
  );
}
if (vueIds.length === 0) {
  console.log("[build:extensions] no Vue extensions to build");
}

for (const id of nativeIds) {
  const manifest = readManifest(id);
  const pkg = manifest?.native?.cargoPackage ?? id;
  console.log("[build:extensions:native]", pkg);
  execFileSync("cargo", ["build", "--release", "-p", pkg], { cwd: repoRoot, stdio: "inherit" });
}
if (nativeIds.length === 0) {
  console.log("[build:extensions:native] no native extensions to build");
}
