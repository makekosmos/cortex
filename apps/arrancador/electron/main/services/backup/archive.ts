import { execFile } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import type {
  BackupArchiveManifest,
  BackupCompressionPreset,
  BackupFileEntry,
} from "./types";
import { createTempDir, ensureDir, isDirectory, isFile, removeBackupArtifact } from "./utils";

const execFileAsync = promisify(execFile);

export const BACKUP_MANIFEST_NAMES = ["__sqoba_manifest.json", "__arrancador_manifest.json"] as const;
export const BACKUP_README_NAME = "__sqoba_readme.txt";
const MANIFEST_VERSION = 2;

function escapePowerShellString(value: string): string {
  return `'${value.replace(/'/g, "''")}'`;
}

async function runPowerShellScript(script: string): Promise<void> {
  if (process.platform !== "win32") {
    throw new Error("ZIP backups require Windows PowerShell");
  }

  const args = ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script];

  try {
    await execFileAsync("powershell.exe", args, {
      windowsHide: true,
      maxBuffer: 10 * 1024 * 1024,
    });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }

    await execFileAsync("pwsh", args, {
      windowsHide: true,
      maxBuffer: 10 * 1024 * 1024,
    });
  }
}

function compressionMode(level: number | undefined): BackupCompressionPreset {
  return (level ?? 60) >= 50 ? "optimal" : "fastest";
}

export function buildBackupManifest(entries: BackupFileEntry[]): BackupArchiveManifest {
  return {
    version: MANIFEST_VERSION,
    files: entries,
  };
}

export async function writeBackupManifestToDirectory(
  destination: string,
  manifest: BackupArchiveManifest,
): Promise<void> {
  await writeFile(path.join(destination, BACKUP_MANIFEST_NAMES[0]), JSON.stringify(manifest, null, 2), "utf8");
}

export async function writeBackupReadmeToDirectory(destination: string): Promise<void> {
  const readme = [
    "SQOBA backup format",
    "",
    "This folder contains raw save files plus a manifest.",
    `- ${BACKUP_MANIFEST_NAMES[0]}: list of files and original paths`,
    "- files/: backed up files in the same structure as the saves",
    "",
    "To restore manually:",
    `1) Open ${BACKUP_MANIFEST_NAMES[0]}`,
    "2) For each entry, copy files/<path> to original_path",
    "",
  ].join("\n");

  await writeFile(path.join(destination, BACKUP_README_NAME), readme, "utf8");
}

export async function compressBackupDirectory(
  destinationZip: string,
  sourceDir: string,
  level?: number,
): Promise<void> {
  await ensureDir(path.dirname(destinationZip));
  await removeBackupArtifact(destinationZip);

  const compression = compressionMode(level);
  const sourcePattern = path.join(sourceDir, "*");
  const script = [
    `Compress-Archive -Path ${escapePowerShellString(sourcePattern)}`,
    `-DestinationPath ${escapePowerShellString(destinationZip)}`,
    `-CompressionLevel ${compression === "optimal" ? "Optimal" : "Fastest"}`,
    "-Force",
  ].join(" ");

  await runPowerShellScript(script);
}

export async function expandBackupArchive(
  archivePath: string,
  destinationDir: string,
): Promise<void> {
  await ensureDir(destinationDir);
  const script = [
    `Expand-Archive -Path ${escapePowerShellString(archivePath)}`,
    `-DestinationPath ${escapePowerShellString(destinationDir)}`,
    "-Force",
  ].join(" ");
  await runPowerShellScript(script);
}

export async function readBackupManifestFromDirectory(
  backupRoot: string,
): Promise<BackupArchiveManifest | null> {
  for (const manifestName of BACKUP_MANIFEST_NAMES) {
    const manifestPath = path.join(backupRoot, manifestName);
    if (!(await isFile(manifestPath))) {
      continue;
    }
    const text = await readFile(manifestPath, "utf8");
    return JSON.parse(text) as BackupArchiveManifest;
  }

  return null;
}

export async function loadBackupManifest(
  backupPath: string,
): Promise<BackupArchiveManifest | null> {
  if (await isDirectory(backupPath)) {
    return readBackupManifestFromDirectory(backupPath);
  }

  if (!(await isFile(backupPath))) {
    return null;
  }

  if (path.extname(backupPath).toLowerCase() !== ".zip") {
    return null;
  }

  const tempDir = await createTempDir("arrancador-manifest-");
  try {
    await expandBackupArchive(backupPath, tempDir);
    return readBackupManifestFromDirectory(tempDir);
  } finally {
    await removeBackupArtifact(tempDir);
  }
}
