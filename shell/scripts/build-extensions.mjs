#!/usr/bin/env node
// Builds all repo extensions:
// - Vue extensions through the shared Vite extension config.
// - Native extensions through Cargo release builds, using `native.devExecutable` for dev-tree runs.

import { execSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const shellRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(shellRoot, "..");
const extensionsRoot = path.join(repoRoot, "extensions");

function readManifest(id) {
  const p = path.join(extensionsRoot, id, "manifest.json");
  if (!existsSync(p)) return null;
  return JSON.parse(readFileSync(p, "utf8"));
}

function extensionIds() {
  if (!existsSync(extensionsRoot)) return [];
  return readdirSync(extensionsRoot).filter((id) =>
    existsSync(path.join(extensionsRoot, id, "manifest.json")),
  );
}

const ids = extensionIds();
const vueIds = ids.filter((id) => readManifest(id)?.kind === "vue");
const nativeIds = ids.filter((id) => readManifest(id)?.kind === "native");

for (const id of vueIds) {
  console.log("[build:extensions]", id);
  execSync(`vite build --configLoader native --config vite.extensions.config.mjs --mode ${id}`, {
    cwd: shellRoot,
    stdio: "inherit",
  });
}
if (vueIds.length === 0) {
  console.log("[build:extensions] no Vue extensions to build");
}

for (const id of nativeIds) {
  const manifest = readManifest(id);
  const pkg = manifest?.native?.cargoPackage ?? id;
  console.log("[build:extensions:native]", pkg);
  execSync(`cargo build --release -p ${pkg}`, { cwd: repoRoot, stdio: "inherit" });
}
if (nativeIds.length === 0) {
  console.log("[build:extensions:native] no native extensions to build");
}
