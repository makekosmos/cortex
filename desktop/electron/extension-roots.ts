import { app } from "electron";
import { existsSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";
import type { ExtensionSource as ExtensionPermissionSource } from "./extension-permissions";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

interface ExtensionRootEntry {
  dir: string;
  source: ExtensionPermissionSource;
}

// Per-id lookup (`resolveExtensionLocation`) обходит цепочку и возвращает первый
// корень, где есть `manifest.json`. Это позволяет смешивать user-installed
// и bundled extensions.
function resolveExtensionRootEntries(): ExtensionRootEntry[] {
  const roots: ExtensionRootEntry[] = [];
  // Repo dev tree: __dirname is platform/desktop/electron/ (or dist-electron/).
  // Source packages can live under products/* or the deprecated extensions/*
  // compatibility root. In packaged builds we skip this branch:
  // otherwise bundled first-party extensions can be misclassified as dev.
  if (!app.isPackaged) {
    const repoRoot = path.resolve(__dirname, "..", "..", "..");
    for (const rootName of ["products", "extensions"]) {
      const dev = path.join(repoRoot, rootName);
      if (existsSync(dev)) roots.push({ dir: dev, source: "dev" });
    }
  }
  const userRoot = path.join(keplerDataDir(), "extensions");
  if (!roots.some((root) => root.dir === userRoot)) roots.push({ dir: userRoot, source: "user" });
  if (process.resourcesPath) {
    const bundled = path.join(process.resourcesPath, "extensions");
    if (!roots.some((root) => root.dir === bundled))
      roots.push({ dir: bundled, source: "bundled" });
  }
  return roots;
}

export function resolveExtensionRoots(): string[] {
  return resolveExtensionRootEntries().map((root) => root.dir);
}

export function extensionUserDataDir(id: string): string {
  return path.join(keplerDataDir(), "extensions-data", id);
}

const USER_DATA_NAME_RE = /^[\w][\w.-]*$/;
const USER_DATA_PATH_SEGMENT_RE = /^[\w][\w.-]*$/;

export function assertSafeUserDataName(name: unknown): asserts name is string {
  if (typeof name !== "string" || !USER_DATA_NAME_RE.test(name)) {
    throw new Error(`[kepler-shell] invalid user data file name: ${String(name)}`);
  }
}

export function resolveSafeUserDataPath(dir: string, name: unknown): string {
  if (typeof name !== "string") {
    throw new Error(`[kepler-shell] invalid user data path: ${String(name)}`);
  }
  const normalized = name.replace(/\\/g, "/");
  if (!normalized || path.isAbsolute(normalized) || normalized.includes("\0")) {
    throw new Error(`[kepler-shell] invalid user data path: ${name}`);
  }
  const parts = normalized.split("/");
  if (
    parts.some((part) => part === "." || part === ".." || !USER_DATA_PATH_SEGMENT_RE.test(part))
  ) {
    throw new Error(`[kepler-shell] invalid user data path: ${name}`);
  }

  const resolvedDir = path.resolve(dir);
  const resolvedPath = path.resolve(resolvedDir, ...parts);
  const relative = path.relative(resolvedDir, resolvedPath);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`[kepler-shell] invalid user data path: ${name}`);
  }
  return resolvedPath;
}

export function ensureUserDataDir(extId: string): string {
  const dir = extensionUserDataDir(extId);
  mkdirSync(dir, { recursive: true });
  return dir;
}

export function resolveExtensionLocation(
  id: string,
): { dir: string; source: ExtensionPermissionSource } | null {
  for (const root of resolveExtensionRootEntries()) {
    const dir = path.join(root.dir, id);
    if (existsSync(path.join(dir, "manifest.json")) || existsSync(path.join(dir, "package.json"))) {
      return { dir, source: root.source };
    }
  }
  return null;
}

export function resolveExtensionDir(id: string): string | null {
  return resolveExtensionLocation(id)?.dir ?? null;
}
