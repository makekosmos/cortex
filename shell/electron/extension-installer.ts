// Install / backup / revert flow для .kext extensions.
//
// .kext = ZIP-архив с обязательным `manifest.json` в root.
//
// Flow:
//   1. Открыть .kext, прочитать manifest.json без full extract.
//   2. Validate: required fields + keplerApiVersion compat check.
//   3. Backup current `extensions/<id>/` → `extensions-backups/<id>/<timestamp>/`
//      (если есть существующая установка). Limit: max 5 backup'ов, старые
//      удаляются.
//   4. Extract .kext в `extensions-tmp/<id>-<stamp>/`, atomic rename →
//      `extensions/<id>/`.
//
// Revert: восстанавливает самый свежий backup (или указанный timestamp) в
// `extensions/<id>/`, текущая копия уходит в новый backup как «pre-revert».

import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";
import { KEPLER_API_VERSION, satisfiesSemver } from "./kepler-api";
import type { ExtensionManifest } from "./extension-host";

// ESM shim — __dirname / __filename не определены в Node ESM bundles
// (extension-installer.ts bundle'ится через vite-plugin-electron в .mjs).
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const MAX_BACKUPS = 5;

interface ZipEntry {
  name: string;
  isDir: boolean;
  data: Buffer;
}

// Дублирует zip-utils.mjs, но в TS и inline'ом — extension-installer
// bundle'ится в Electron main process через vite-plugin-electron, поэтому
// нельзя просто require'нуть .mjs sibling. Это та же логика, just TS.
//
// 100 строк zip-reader'а — приемлемая цена за то, чтобы не тащить yauzl
// и не воевать с native-биндингами в Electron.

import zlib from "node:zlib";

const EOCD_SIG = 0x06054b50;
const CDFH_SIG = 0x02014b50;
const LFH_SIG = 0x04034b50;

function findEOCD(buf: Buffer): { cdOffset: number; cdEntries: number } | null {
  const minOffset = Math.max(0, buf.length - 65557);
  for (let i = buf.length - 22; i >= minOffset; i--) {
    if (buf.readUInt32LE(i) === EOCD_SIG) {
      const cdEntries = buf.readUInt16LE(i + 10);
      const cdOffset = buf.readUInt32LE(i + 16);
      const cdSize = buf.readUInt32LE(i + 12);
      if (
        cdEntries === 0xffff ||
        cdOffset === 0xffffffff ||
        cdSize === 0xffffffff
      ) {
        throw new Error("zip: ZIP64 not supported");
      }
      return { cdOffset, cdEntries };
    }
  }
  return null;
}

function readZipEntries(zipPath: string): ZipEntry[] {
  const buf = readFileSync(zipPath);
  const eocd = findEOCD(buf);
  if (!eocd) throw new Error(`not a valid zip (no EOCD): ${zipPath}`);
  const entries: ZipEntry[] = [];
  let offset = eocd.cdOffset;
  for (let i = 0; i < eocd.cdEntries; i++) {
    if (buf.readUInt32LE(offset) !== CDFH_SIG) {
      throw new Error(`zip: bad central dir header at ${offset}`);
    }
    const compMethod = buf.readUInt16LE(offset + 10);
    const compSize = buf.readUInt32LE(offset + 20);
    const uncompSize = buf.readUInt32LE(offset + 24);
    const nameLen = buf.readUInt16LE(offset + 28);
    const extraLen = buf.readUInt16LE(offset + 30);
    const commentLen = buf.readUInt16LE(offset + 32);
    const lfhOffset = buf.readUInt32LE(offset + 42);
    const name = buf.subarray(offset + 46, offset + 46 + nameLen).toString("utf8");
    if (buf.readUInt32LE(lfhOffset) !== LFH_SIG) {
      throw new Error(`zip: bad local header at ${lfhOffset} for ${name}`);
    }
    const lfhNameLen = buf.readUInt16LE(lfhOffset + 26);
    const lfhExtraLen = buf.readUInt16LE(lfhOffset + 28);
    const dataStart = lfhOffset + 30 + lfhNameLen + lfhExtraLen;
    const rawData = buf.subarray(dataStart, dataStart + compSize);
    let data: Buffer;
    if (name.endsWith("/") || uncompSize === 0) {
      data = Buffer.alloc(0);
    } else if (compMethod === 0) {
      data = Buffer.from(rawData);
    } else if (compMethod === 8) {
      data = zlib.inflateRawSync(rawData);
    } else {
      throw new Error(`zip: unsupported compression ${compMethod} for ${name}`);
    }
    entries.push({ name, isDir: name.endsWith("/"), data });
    offset += 46 + nameLen + extraLen + commentLen;
  }
  return entries;
}

function safeEntryName(name: string): string {
  if (!name) throw new Error("zip: empty entry name");
  const norm = name.replace(/\\/g, "/");
  if (norm.startsWith("/")) throw new Error(`zip: absolute path: ${name}`);
  if (/^[a-zA-Z]:/.test(norm)) throw new Error(`zip: drive letter: ${name}`);
  for (const p of norm.split("/")) {
    if (p === "..") throw new Error(`zip: parent traversal: ${name}`);
  }
  return norm;
}

function extractZipTo(zipPath: string, targetDir: string): void {
  const entries = readZipEntries(zipPath);
  mkdirSync(targetDir, { recursive: true });
  for (const e of entries) {
    const safe = safeEntryName(e.name);
    const out = path.join(targetDir, safe);
    if (e.isDir) {
      mkdirSync(out, { recursive: true });
      continue;
    }
    mkdirSync(path.dirname(out), { recursive: true });
    writeFileSync(out, e.data);
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Manifest preview данные для install dialog — то что показывается
 * пользователю ДО подтверждения установки.
 */
export interface KextManifestPreview {
  manifest: ExtensionManifest;
  iconDataUri: string | null;
  apiCompatError: string | null;
  /** Существует ли уже установленная копия с тем же id. */
  isUpgrade: boolean;
  /** Версия существующей установки (если есть). */
  currentVersion: string | null;
}

export function userExtensionsRoot(): string {
  return path.join(keplerDataDir(), "extensions");
}

export function extensionsBackupsRoot(): string {
  return path.join(keplerDataDir(), "extensions-backups");
}

function extensionsTmpRoot(): string {
  return path.join(keplerDataDir(), "extensions-tmp");
}

/**
 * Читает manifest.json из .kext без full extract'а. Также пытается прочесть
 * icon если указан в manifest. Возвращает preview данные для dialog.
 *
 * Throws при invalid zip / отсутствии manifest.json / invalid JSON.
 */
export function previewKext(kextPath: string): KextManifestPreview {
  const entries = readZipEntries(kextPath);
  const manifestEntry = entries.find(
    (e) => e.name === "manifest.json" && !e.isDir,
  );
  if (!manifestEntry) {
    throw new Error("manifest.json не найден в .kext");
  }
  let manifest: ExtensionManifest;
  try {
    manifest = JSON.parse(manifestEntry.data.toString("utf8")) as ExtensionManifest;
  } catch (e) {
    throw new Error(`manifest.json повреждён: ${(e as Error).message}`);
  }
  if (!manifest.id || typeof manifest.id !== "string") {
    throw new Error("manifest.id обязателен и должен быть строкой");
  }
  if (!/^[\w][\w.-]*$/.test(manifest.id)) {
    throw new Error(`manifest.id невалиден: ${manifest.id}`);
  }
  if (!manifest.name || typeof manifest.name !== "string") {
    throw new Error("manifest.name обязателен");
  }

  let apiCompatError: string | null = null;
  if (manifest.keplerApiVersion) {
    if (!satisfiesSemver(KEPLER_API_VERSION, manifest.keplerApiVersion)) {
      apiCompatError =
        `Расширение требует Kepler API ${manifest.keplerApiVersion}, ` +
        `установлено ${KEPLER_API_VERSION}.`;
    }
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

  // Существующая установка?
  const currentDir = path.join(userExtensionsRoot(), manifest.id);
  let isUpgrade = false;
  let currentVersion: string | null = null;
  if (existsSync(path.join(currentDir, "manifest.json"))) {
    isUpgrade = true;
    try {
      const cur = JSON.parse(
        readFileSync(path.join(currentDir, "manifest.json"), "utf8"),
      ) as ExtensionManifest;
      currentVersion = cur.version ?? null;
    } catch {
      /* ignore */
    }
  }

  return { manifest, iconDataUri, apiCompatError, isUpgrade, currentVersion };
}

/**
 * Если .kext — путь к существующему файлу с расширением .kext или .zip:
 * traited as zip. Если path — директория с manifest.json: traited as
 * pre-extracted (для dev). previewKext поддерживает только zip — для dir
 * используем previewDir.
 */
export function previewDir(extDir: string): KextManifestPreview {
  const manifestPath = path.join(extDir, "manifest.json");
  if (!existsSync(manifestPath)) {
    throw new Error(`manifest.json не найден в ${extDir}`);
  }
  let manifest: ExtensionManifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as ExtensionManifest;
  } catch (e) {
    throw new Error(`manifest.json повреждён: ${(e as Error).message}`);
  }
  if (!manifest.id || !manifest.name) {
    throw new Error("manifest.id и manifest.name обязательны");
  }
  let apiCompatError: string | null = null;
  if (manifest.keplerApiVersion) {
    if (!satisfiesSemver(KEPLER_API_VERSION, manifest.keplerApiVersion)) {
      apiCompatError =
        `Расширение требует Kepler API ${manifest.keplerApiVersion}, ` +
        `установлено ${KEPLER_API_VERSION}.`;
    }
  }
  let iconDataUri: string | null = null;
  if (manifest.icon) {
    const iconPath = path.join(extDir, manifest.icon);
    if (existsSync(iconPath)) {
      const ext = path.extname(manifest.icon).toLowerCase();
      const mime =
        ext === ".svg"
          ? "image/svg+xml"
          : ext === ".jpg" || ext === ".jpeg"
            ? "image/jpeg"
            : "image/png";
      iconDataUri = `data:${mime};base64,${readFileSync(iconPath).toString("base64")}`;
    }
  }
  const currentDir = path.join(userExtensionsRoot(), manifest.id);
  let isUpgrade = false;
  let currentVersion: string | null = null;
  if (existsSync(path.join(currentDir, "manifest.json"))) {
    isUpgrade = true;
    try {
      const cur = JSON.parse(
        readFileSync(path.join(currentDir, "manifest.json"), "utf8"),
      ) as ExtensionManifest;
      currentVersion = cur.version ?? null;
    } catch {
      /* ignore */
    }
  }
  return { manifest, iconDataUri, apiCompatError, isUpgrade, currentVersion };
}

/**
 * Универсальный preview: если path — .kext / .zip файл, читает zip; если
 * директория с manifest.json, читает её. Используется install dialog'ом.
 */
export function previewSource(sourcePath: string): KextManifestPreview {
  if (!existsSync(sourcePath)) {
    throw new Error(`источник не существует: ${sourcePath}`);
  }
  const stat = statSync(sourcePath);
  if (stat.isDirectory()) {
    return previewDir(sourcePath);
  }
  return previewKext(sourcePath);
}

/**
 * Бэкап текущей `extensions/<id>/` в `extensions-backups/<id>/<timestamp>/`.
 * Возвращает path к созданному backup'у или null если бэкапить нечего.
 *
 * После создания удаляет старые backup'ы, если их больше MAX_BACKUPS.
 */
export function backupExtension(id: string): string | null {
  const currentDir = path.join(userExtensionsRoot(), id);
  if (!existsSync(currentDir)) return null;
  const backupsRoot = path.join(extensionsBackupsRoot(), id);
  mkdirSync(backupsRoot, { recursive: true });
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
  const target = path.join(backupsRoot, timestamp);
  // cpSync с recursive — Node 16+ поддерживает.
  copyDirSync(currentDir, target);
  // Pruning: удалить старые backup'ы.
  pruneBackups(id);
  return target;
}

function copyDirSync(src: string, dst: string): void {
  mkdirSync(dst, { recursive: true });
  for (const entry of readdirSync(src, { withFileTypes: true })) {
    const s = path.join(src, entry.name);
    const d = path.join(dst, entry.name);
    if (entry.isDirectory()) {
      copyDirSync(s, d);
    } else if (entry.isFile()) {
      writeFileSync(d, readFileSync(s));
    }
    // Symlinks / sockets игнорируем — extension'ы не должны их содержать.
  }
}

function pruneBackups(id: string): void {
  const root = path.join(extensionsBackupsRoot(), id);
  if (!existsSync(root)) return;
  const entries = readdirSync(root)
    .filter((n) => {
      try {
        return statSync(path.join(root, n)).isDirectory();
      } catch {
        return false;
      }
    })
    .sort();
  // Старшие в начале (ISO timestamp сортится лексикографически).
  while (entries.length > MAX_BACKUPS) {
    const old = entries.shift()!;
    try {
      rmSync(path.join(root, old), { recursive: true, force: true });
    } catch (e) {
      console.warn(`[ext:install] prune backup ${id}/${old} failed:`, e);
    }
  }
}

export function listBackups(id: string): string[] {
  const root = path.join(extensionsBackupsRoot(), id);
  if (!existsSync(root)) return [];
  return readdirSync(root)
    .filter((n) => {
      try {
        return statSync(path.join(root, n)).isDirectory();
      } catch {
        return false;
      }
    })
    .sort()
    .reverse(); // самые свежие первыми
}

/**
 * Устанавливает .kext / extension dir в `extensions/<id>/`.
 *
 * 1. Preview (re-validate перед extract'ом).
 * 2. Backup current если есть.
 * 3. Extract в tmp dir + atomic rename → target.
 *
 * При ошибке после backup'а — best-effort откатывает на backup.
 *
 * Возвращает финальный preview с обновлёнными currentVersion / isUpgrade.
 */
export function installFromPath(sourcePath: string): KextManifestPreview {
  const preview = previewSource(sourcePath);
  if (preview.apiCompatError) {
    throw new Error(`API compat: ${preview.apiCompatError}`);
  }
  const id = preview.manifest.id;
  const tmpRoot = extensionsTmpRoot();
  mkdirSync(tmpRoot, { recursive: true });
  const stamp = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
  const tmpDir = path.join(tmpRoot, `${id}-${stamp}`);

  try {
    const stat = statSync(sourcePath);
    if (stat.isDirectory()) {
      copyDirSync(sourcePath, tmpDir);
    } else {
      extractZipTo(sourcePath, tmpDir);
    }
    // Validate post-extract: manifest.json должен совпасть с тем, что мы
    // прочитали в preview (id матчится).
    const extractedManifest = JSON.parse(
      readFileSync(path.join(tmpDir, "manifest.json"), "utf8"),
    ) as ExtensionManifest;
    if (extractedManifest.id !== id) {
      throw new Error(
        `manifest.id после extract'а (${extractedManifest.id}) не совпадает с preview (${id})`,
      );
    }
  } catch (e) {
    rmSync(tmpDir, { recursive: true, force: true });
    throw e;
  }

  // Бэкап.
  const backup = backupExtension(id);

  const target = path.join(userExtensionsRoot(), id);
  mkdirSync(path.dirname(target), { recursive: true });

  try {
    // Atomic switch: rename target → .old, rename tmp → target, rm .old.
    let oldDir: string | null = null;
    if (existsSync(target)) {
      oldDir = `${target}.old-${stamp}`;
      renameSync(target, oldDir);
    }
    try {
      renameSync(tmpDir, target);
    } catch (e) {
      // rollback
      if (oldDir && existsSync(oldDir) && !existsSync(target)) {
        renameSync(oldDir, target);
      }
      throw e;
    }
    if (oldDir && existsSync(oldDir)) {
      rmSync(oldDir, { recursive: true, force: true });
    }
  } catch (e) {
    // best-effort revert from backup
    if (backup && !existsSync(target)) {
      try {
        copyDirSync(backup, target);
      } catch (revertErr) {
        console.error(`[ext:install] revert from backup failed:`, revertErr);
      }
    }
    rmSync(tmpDir, { recursive: true, force: true });
    throw e;
  }

  // Возвращаем обновлённый preview (после install).
  const finalPreview = previewDir(target);
  return finalPreview;
}

/**
 * Восстанавливает extension из backup'а. Перед revert'ом текущая копия
 * сохраняется как новый backup — revert reversible.
 *
 * Если `timestamp` не указан — берётся самый свежий backup. Возвращает true
 * если revert удался, false если backup'ов нет.
 */
export function revertExtension(id: string, timestamp?: string): boolean {
  const backups = listBackups(id);
  if (backups.length === 0) return false;
  const chosen = timestamp ?? backups[0]!;
  const backupDir = path.join(extensionsBackupsRoot(), id, chosen);
  if (!existsSync(path.join(backupDir, "manifest.json"))) {
    throw new Error(`backup not found or corrupt: ${id}/${chosen}`);
  }
  const target = path.join(userExtensionsRoot(), id);

  // Сохраняем текущую копию как pre-revert backup (если есть).
  if (existsSync(target)) {
    backupExtension(id);
  }

  const stamp = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
  const tmpRoot = extensionsTmpRoot();
  mkdirSync(tmpRoot, { recursive: true });
  const tmpDir = path.join(tmpRoot, `${id}-revert-${stamp}`);

  try {
    copyDirSync(backupDir, tmpDir);
    let oldDir: string | null = null;
    if (existsSync(target)) {
      oldDir = `${target}.old-${stamp}`;
      renameSync(target, oldDir);
    }
    try {
      renameSync(tmpDir, target);
    } catch (e) {
      if (oldDir && !existsSync(target)) renameSync(oldDir, target);
      throw e;
    }
    if (oldDir && existsSync(oldDir)) {
      rmSync(oldDir, { recursive: true, force: true });
    }
    return true;
  } catch (e) {
    rmSync(tmpDir, { recursive: true, force: true });
    throw e;
  }
}

/**
 * Список installed user-extensions с metadata для Settings UI.
 */
export interface InstalledExtensionInfo {
  id: string;
  name: string;
  version: string | null;
  description: string | null;
  author: string | null;
  iconDataUri: string | null;
  backupCount: number;
  backupTimestamps: string[];
  /** "installed" — user-installed в <dataDir>/extensions/<id>/.
   *  "dev" — repo dev tree, auto-detect'ится при запуске из repo. */
  source: "installed" | "dev";
}

/** Repo dev tree root для extensions. Если папка существует — мы запущены
 *  из repo (developer flow), и extension'ы оттуда автоматически считаются
 *  "installed". Это устраняет нужду качать extension с marketplace в dev. */
function repoDevExtensionsRoot(): string | null {
  // shell/electron/ → shell/ → <repoRoot>/, extensions at <repoRoot>/extensions/
  const candidate = path.resolve(__dirname, "..", "..", "extensions");
  return existsSync(candidate) ? candidate : null;
}

function readManifestSafe(dir: string): ExtensionManifest | null {
  const manifestPath = path.join(dir, "manifest.json");
  if (!existsSync(manifestPath)) return null;
  try {
    return JSON.parse(readFileSync(manifestPath, "utf8")) as ExtensionManifest;
  } catch {
    return null;
  }
}

function readIconDataUri(dir: string, manifest: ExtensionManifest): string | null {
  if (!manifest.icon) return null;
  const iconPath = path.join(dir, manifest.icon);
  if (!existsSync(iconPath)) return null;
  const ext = path.extname(manifest.icon).toLowerCase();
  const mime =
    ext === ".svg"
      ? "image/svg+xml"
      : ext === ".jpg" || ext === ".jpeg"
        ? "image/jpeg"
        : "image/png";
  return `data:${mime};base64,${readFileSync(iconPath).toString("base64")}`;
}

function scanExtensionsDir(
  root: string,
  source: "installed" | "dev",
): InstalledExtensionInfo[] {
  if (!existsSync(root)) return [];
  const out: InstalledExtensionInfo[] = [];
  for (const id of readdirSync(root)) {
    const dir = path.join(root, id);
    let stat;
    try {
      stat = statSync(dir);
    } catch {
      continue;
    }
    if (!stat.isDirectory()) continue;
    const manifest = readManifestSafe(dir);
    if (!manifest) continue;
    // Dev-source extensions могут быть в repo но без built dist/ —
    // не показываем их как "installed" пока bun run build:extensions не сделан.
    // Иначе UI повёл бы юзера в landing где openExtension падает на "entryHtml not found".
    if (source === "dev") {
      const entryHtml = manifest.entryHtml ?? "dist/index.html";
      const entryPath = path.join(dir, entryHtml);
      if (!existsSync(entryPath)) continue;
    }
    const iconDataUri = readIconDataUri(dir, manifest);
    // backupCount только для installed — у dev-source это repo state, revert не имеет смысла.
    const backups = source === "installed" ? listBackups(id) : [];
    out.push({
      id,
      name: manifest.name,
      version: manifest.version ?? null,
      description: manifest.description ?? null,
      author: manifest.author ?? null,
      iconDataUri,
      backupCount: backups.length,
      backupTimestamps: backups,
      source,
    });
  }
  return out;
}

export function listInstalledUserExtensions(): InstalledExtensionInfo[] {
  // Resolution priority согласована с extension-host.ts::resolveExtensionRoots:
  //   1. Repo dev tree — если запущены из repo (developer flow).
  //   2. User-installed — production flow через marketplace / .kext install.
  // Dev shadow'ит installed (dedup по id): если extension есть и в repo и
  // в %APPDATA%, в списке показывается repo version с source="dev".
  // Это значит в dev mode user видит свои repo extensions как "installed"
  // и launcher commands работают сразу — без marketplace download.
  const dev = repoDevExtensionsRoot();
  const devList = dev ? scanExtensionsDir(dev, "dev") : [];
  const installedList = scanExtensionsDir(userExtensionsRoot(), "installed");
  const seen = new Set(devList.map((e) => e.id));
  const merged = [
    ...devList,
    ...installedList.filter((e) => !seen.has(e.id)),
  ];
  return merged;
}

/**
 * Удаляет user-installed extension (как `ext:uninstall <id>`). User data
 * (`extensions-data/<id>/`) сохраняется. Возвращает true если что-то
 * удалили, false если код-папка не существовала.
 */
export function uninstallExtension(id: string): boolean {
  const dir = path.join(userExtensionsRoot(), id);
  if (!existsSync(dir)) return false;
  rmSync(dir, { recursive: true, force: true });
  return true;
}
