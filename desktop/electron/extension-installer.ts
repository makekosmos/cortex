// Install flow for .kext extensions.
//
// .kext = ZIP archive with a required manifest.json at root.

import { existsSync, renameSync } from "node:fs";
import * as fs from "node:fs/promises";
import { app } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";
import { KEPLER_API_VERSION, satisfiesSemver } from "./kepler-api";
import {
  backupExtension,
  listBackups as listBackupsState,
  listInstalledUserExtensions as listInstalledUserExtensionsState,
  revertExtension as revertExtensionState,
  uninstallExtension as uninstallExtensionState,
} from "./extension-installer-state";
import type { InstalledExtensionInfo } from "./extension-installer-state";
import { extractZipTo, readZipEntries } from "./extension-zip";
import type { JsonRecord } from "../src/shared/runtimeGuards";
import { validateExtensionManifest, type ExtensionManifest } from "./extension-manifest-validation";
import {
  assertLegacyLaunchAllowed,
  legacyMigrationTarget,
  withLegacyMigrationLock,
} from "./legacy-migration-journal";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export type { InstalledExtensionInfo } from "./extension-installer-state";

export interface KextManifestPreview {
  manifest: ExtensionManifest;
  iconDataUri: string | null;
  apiCompatError: string | null;
  isUpgrade: boolean;
  currentVersion: string | null;
}

function userExtensionsRoot(): string {
  return path.join(keplerDataDir(), "extensions");
}

function extensionsTmpRoot(): string {
  return path.join(keplerDataDir(), "extensions-tmp");
}

async function previewKext(kextPath: string): Promise<KextManifestPreview> {
  const entries = await readZipEntries(kextPath);
  const manifestEntry = entries.find((e) => e.name === "manifest.json" && !e.isDir);
  if (!manifestEntry) {
    throw new Error("manifest.json не найден в .kext");
  }
  let manifest: ExtensionManifest;
  try {
    // SAFETY: validateExtensionManifest rejects every non-object JSON value at runtime.
    const parsed = JSON.parse(manifestEntry.data.toString("utf8")) as JsonRecord | null;
    manifest = validateExtensionManifest(parsed);
  } catch (e) {
    // SAFETY: JSON.parse errors are Error instances in the Node runtime.
    throw new Error(`manifest.json повреждён: ${(e as Error).message}`);
  }
  let apiCompatError: string | null = null;
  if (
    manifest.keplerApiVersion &&
    !satisfiesSemver(KEPLER_API_VERSION, manifest.keplerApiVersion)
  ) {
    apiCompatError =
      `Расширение требует Kepler API ${manifest.keplerApiVersion}, ` +
      `установлено ${KEPLER_API_VERSION}.`;
  }

  let iconDataUri: string | null = null;
  if (manifest.icon) {
    const iconEntry = entries.find((e) => e.name === manifest.icon && !e.isDir);
    if (iconEntry) {
      const ext = path.extname(manifest.icon).toLowerCase();
      const mime =
        ext === ".svg"
          ? "image/svg+xml"
          : ext === ".jpg" || ext === ".jpeg"
            ? "image/jpeg"
            : "image/png";
      iconDataUri = `data:${mime};base64,${iconEntry.data.toString("base64")}`;
    }
  }

  const currentDir = path.join(userExtensionsRoot(), manifest.id);
  let isUpgrade = false;
  let currentVersion: string | null = null;
  if (existsSync(path.join(currentDir, "manifest.json"))) {
    isUpgrade = true;
    try {
      // SAFETY: the stored manifest is validated when the extension is installed.
      const cur = JSON.parse(
        await fs.readFile(path.join(currentDir, "manifest.json"), "utf8"),
      ) as ExtensionManifest;
      currentVersion = cur.version ?? null;
    } catch {
      /* ignore */
    }
  }

  return { manifest, iconDataUri, apiCompatError, isUpgrade, currentVersion };
}

async function previewDir(extDir: string): Promise<KextManifestPreview> {
  const manifestPath = path.join(extDir, "manifest.json");
  if (!existsSync(manifestPath)) {
    throw new Error(`manifest.json не найден в ${extDir}`);
  }
  let manifest: ExtensionManifest;
  try {
    // SAFETY: validateExtensionManifest rejects every non-object JSON value at runtime.
    const parsed = JSON.parse(await fs.readFile(manifestPath, "utf8")) as JsonRecord | null;
    manifest = validateExtensionManifest(parsed);
  } catch (e) {
    // SAFETY: JSON.parse errors are Error instances in the Node runtime.
    throw new Error(`manifest.json повреждён: ${(e as Error).message}`);
  }
  let apiCompatError: string | null = null;
  if (
    manifest.keplerApiVersion &&
    !satisfiesSemver(KEPLER_API_VERSION, manifest.keplerApiVersion)
  ) {
    apiCompatError =
      `Расширение требует Kepler API ${manifest.keplerApiVersion}, ` +
      `установлено ${KEPLER_API_VERSION}.`;
  }
  let iconDataUri: string | null = null;
  if (manifest.icon) {
    const iconPath = path.join(extDir, manifest.icon);
    if (existsSync(iconPath)) {
      const [packageRoot, realIconPath] = await Promise.all([
        fs.realpath(extDir),
        fs.realpath(iconPath),
      ]);
      const relativeIconPath = path.relative(packageRoot, realIconPath);
      if (relativeIconPath.startsWith(`..${path.sep}`) || path.isAbsolute(relativeIconPath)) {
        throw new Error("manifest.icon выходит за пределы пакета");
      }
      const ext = path.extname(manifest.icon).toLowerCase();
      const mime =
        ext === ".svg"
          ? "image/svg+xml"
          : ext === ".jpg" || ext === ".jpeg"
            ? "image/jpeg"
            : "image/png";
      iconDataUri = `data:${mime};base64,${(await fs.readFile(realIconPath)).toString("base64")}`;
    }
  }
  const currentDir = path.join(userExtensionsRoot(), manifest.id);
  let isUpgrade = false;
  let currentVersion: string | null = null;
  if (existsSync(path.join(currentDir, "manifest.json"))) {
    isUpgrade = true;
    try {
      // SAFETY: the stored manifest is validated when the extension is installed.
      const cur = JSON.parse(
        await fs.readFile(path.join(currentDir, "manifest.json"), "utf8"),
      ) as ExtensionManifest;
      currentVersion = cur.version ?? null;
    } catch {
      /* ignore */
    }
  }
  return { manifest, iconDataUri, apiCompatError, isUpgrade, currentVersion };
}

export async function previewSource(sourcePath: string): Promise<KextManifestPreview> {
  if (!existsSync(sourcePath)) {
    throw new Error(`источник не существует: ${sourcePath}`);
  }
  const stat = await fs.stat(sourcePath);
  if (stat.isDirectory()) return previewDir(sourcePath);
  return previewKext(sourcePath);
}

function repoDevExtensionsRoot(): string | null {
  if (app.isPackaged) return null;
  const candidate = path.resolve(__dirname, "..", "..", "..", "extensions");
  return existsSync(candidate) ? candidate : null;
}

export async function listBackups(id: string): Promise<string[]> {
  return listBackupsState(id);
}

export async function revertExtension(id: string, timestamp?: string): Promise<boolean> {
  const target = legacyMigrationTarget(id);
  const revert = () => {
    assertLegacyLaunchAllowed(keplerDataDir(), id);
    return revertExtensionState(id, timestamp);
  };
  return target ? withLegacyMigrationLock(keplerDataDir(), target, revert) : revert();
}

export async function listInstalledUserExtensions(): Promise<InstalledExtensionInfo[]> {
  return listInstalledUserExtensionsState(userExtensionsRoot(), repoDevExtensionsRoot());
}

export async function uninstallExtension(id: string): Promise<boolean> {
  const target = legacyMigrationTarget(id);
  const uninstall = () => {
    assertLegacyLaunchAllowed(keplerDataDir(), id);
    return uninstallExtensionState(userExtensionsRoot(), id);
  };
  return target ? withLegacyMigrationLock(keplerDataDir(), target, uninstall) : uninstall();
}

export async function installFromPath(sourcePath: string): Promise<KextManifestPreview> {
  const preview = await previewSource(sourcePath);
  const target = legacyMigrationTarget(preview.manifest.id);
  const install = () => installFromPathUnlocked(sourcePath, preview);
  if (!target) {
    assertLegacyLaunchAllowed(keplerDataDir(), preview.manifest.id);
    return install();
  }
  return withLegacyMigrationLock(keplerDataDir(), target, async () => {
    assertLegacyLaunchAllowed(keplerDataDir(), preview.manifest.id);
    return install();
  });
}

async function installFromPathUnlocked(
  sourcePath: string,
  preview: KextManifestPreview,
): Promise<KextManifestPreview> {
  if (preview.apiCompatError) {
    throw new Error(`API compat: ${preview.apiCompatError}`);
  }
  const id = preview.manifest.id;
  const tmpRoot = extensionsTmpRoot();
  await fs.mkdir(tmpRoot, { recursive: true });
  const stamp = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
  const tmpDir = path.join(tmpRoot, `${id}-${stamp}`);

  try {
    const stat = await fs.stat(sourcePath);
    if (stat.isDirectory()) {
      await fs.cp(sourcePath, tmpDir, { recursive: true });
    } else {
      await extractZipTo(sourcePath, tmpDir);
    }
    // SAFETY: the extracted archive contains the manifest validated during preview.
    const extractedManifest = JSON.parse(
      await fs.readFile(path.join(tmpDir, "manifest.json"), "utf8"),
    ) as ExtensionManifest;
    if (extractedManifest.id !== id) {
      throw new Error(
        `manifest.id после extract'а (${extractedManifest.id}) не совпадает с preview (${id})`,
      );
    }
  } catch (e) {
    await fs.rm(tmpDir, { recursive: true, force: true });
    throw e;
  }

  const backup = await backupExtension(id);
  const target = path.join(userExtensionsRoot(), id);
  await fs.mkdir(path.dirname(target), { recursive: true });

  try {
    let oldDir: string | null = null;
    if (existsSync(target)) {
      oldDir = `${target}.old-${stamp}`;
      renameSync(target, oldDir);
    }
    try {
      renameSync(tmpDir, target);
    } catch (e) {
      if (oldDir && existsSync(oldDir) && !existsSync(target)) {
        renameSync(oldDir, target);
      }
      throw e;
    }
    if (oldDir && existsSync(oldDir)) {
      await fs.rm(oldDir, { recursive: true, force: true });
    }
  } catch (e) {
    if (backup && !existsSync(target)) {
      try {
        await fs.cp(backup, target, { recursive: true });
      } catch (revertErr) {
        console.error(`[ext:install] revert from backup failed:`, revertErr);
      }
    }
    await fs.rm(tmpDir, { recursive: true, force: true });
    throw e;
  }

  return previewDir(target);
}
