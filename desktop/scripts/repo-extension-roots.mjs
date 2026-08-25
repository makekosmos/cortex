import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";

// Source-tree roots for first-party and experimental app packages.
// User-installed runtime extensions still live in <dataDir>/extensions.
const REPO_EXTENSION_ROOT_NAMES = ["products", "extensions"];
const SIBLING_EXTENSION_REPOS = ["memoria", "agenda", "arcadia", "dictation"];

function repoExtensionRoots(repoRoot) {
  const monorepoRoots = REPO_EXTENSION_ROOT_NAMES.map((name) => ({
    name,
    dir: path.join(repoRoot, name),
  }));
  const siblingRoots = SIBLING_EXTENSION_REPOS.map((name) => ({
    name,
    dir: path.join(repoRoot, name),
    standalone: true,
  }));
  return [...monorepoRoots, ...siblingRoots].filter((root) => existsSync(root.dir));
}

function readManifestSafe(dir) {
  const manifestPath = path.join(dir, "manifest.json");
  if (!existsSync(manifestPath)) return null;
  try {
    return JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch {
    return null;
  }
}

export function listRepoExtensionEntries(repoRoot) {
  const out = [];
  const seen = new Set();
  for (const root of repoExtensionRoots(repoRoot)) {
    if (root.standalone) {
      const manifest = readManifestSafe(root.dir);
      if (manifest?.id && Object.prototype.toString.call(manifest.id) === "[object String]" && !seen.has(manifest.id)) {
        seen.add(manifest.id);
        out.push({ id: manifest.id, folder: root.name, dir: root.dir, rootName: root.name, manifest });
      }
      continue;
    }
    for (const entry of readdirSync(root.dir, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const dir = path.join(root.dir, entry.name);
      const manifest = readManifestSafe(dir);
      if (!manifest?.id || Object.prototype.toString.call(manifest.id) !== "[object String]") continue;
      if (seen.has(manifest.id)) continue;
      seen.add(manifest.id);
      out.push({ id: manifest.id, folder: entry.name, dir, rootName: root.name, manifest });
    }
  }
  return out;
}

export function findRepoExtensionEntry(repoRoot, id) {
  return (
    listRepoExtensionEntries(repoRoot).find((entry) => entry.id === id || entry.folder === id) ??
    null
  );
}
