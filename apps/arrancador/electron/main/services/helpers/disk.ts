import { execFile } from "node:child_process";
import { mkdtemp, open, rm, stat } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import type { DiskSpeedResult, SystemDiskInfo } from "../contracts";

const execFileAsync = promisify(execFile);
const TEST_FILE_SIZE = 128 * 1024 * 1024;
const CHUNK_SIZE = 4 * 1024 * 1024;

function outputToUtf8(value: string | Buffer) {
  return typeof value === "string" ? value : value.toString("utf8");
}

function toBigIntBytes(value: unknown) {
  if (typeof value === "number" && Number.isFinite(value)) {
    return Math.max(0, Math.trunc(value));
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number.parseInt(value, 10);
    return Number.isNaN(parsed) ? 0 : Math.max(0, parsed);
  }
  return 0;
}

function formatDiskKind(value: unknown) {
  const driveType = toBigIntBytes(value);
  switch (driveType) {
    case 2:
      return "Removable";
    case 3:
      return "Fixed";
    case 4:
      return "Network";
    case 5:
      return "CD-ROM";
    case 6:
      return "RAM";
    default:
      return "Unknown";
  }
}

async function collectWindowsDisks(): Promise<SystemDiskInfo[]> {
  try {
    const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
Get-CimInstance Win32_LogicalDisk |
  Select-Object DeviceID, VolumeName, FileSystem, Size, FreeSpace, DriveType |
  ConvertTo-Json -Depth 4 -Compress
`.trim();
    const { stdout } = await execFileAsync(
      "powershell.exe",
      ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script],
      { maxBuffer: 16 * 1024 * 1024, windowsHide: true },
    );

    const text = outputToUtf8(stdout).trim();
    const rows = text ? (JSON.parse(text) as unknown) : [];
    const list = Array.isArray(rows) ? rows : [rows];

    return list
      .map((row) => {
        if (!row || typeof row !== "object") {
          return null;
        }
        const entry = row as Record<string, unknown>;
        const mountPoint = typeof entry.DeviceID === "string" ? entry.DeviceID : "";
        if (!mountPoint) {
          return null;
        }

        const fileSystem =
          typeof entry.FileSystem === "string" && entry.FileSystem
            ? entry.FileSystem
            : "Unknown";
        const volumeName =
          typeof entry.VolumeName === "string" && entry.VolumeName
            ? entry.VolumeName
            : null;

        return {
          name: volumeName ?? mountPoint,
          mount_point: mountPoint,
          file_system: fileSystem,
          total_bytes: toBigIntBytes(entry.Size),
          available_bytes: toBigIntBytes(entry.FreeSpace),
          kind: formatDiskKind(entry.DriveType),
          is_removable: toBigIntBytes(entry.DriveType) === 2,
          model: volumeName,
          media_type: null,
        } satisfies SystemDiskInfo;
      })
      .filter((entry): entry is SystemDiskInfo => Boolean(entry))
      .sort((a, b) => a.mount_point.localeCompare(b.mount_point));
  } catch {
    return [];
  }
}

async function getFilesystemType(mountPoint: string) {
  try {
    const args =
      process.platform === "darwin"
        ? ["-f", "%T", mountPoint]
        : ["-f", "-c", "%T", mountPoint];
    const { stdout } = await execFileAsync("stat", args, {
      maxBuffer: 1024 * 1024,
    });
    const value = outputToUtf8(stdout).trim();
    return value || "Unknown";
  } catch {
    return "Unknown";
  }
}

async function collectPosixDisks(): Promise<SystemDiskInfo[]> {
  try {
    const { stdout } = await execFileAsync("df", ["-Pk"], {
      maxBuffer: 16 * 1024 * 1024,
    });

    const lines = outputToUtf8(stdout)
      .split(/\r?\n/)
      .slice(1)
      .map((line) => line.trim())
      .filter(Boolean);

    const rows = lines
      .map((line) => {
        const match = /^(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+\d+%\s+(.+)$/.exec(line);
        if (!match) {
          return null;
        }

        return {
          source: match[1],
          totalBlocks: Number(match[2]),
          availableBlocks: Number(match[4]),
          mountPoint: match[5],
        };
      })
      .filter(
        (
          row,
        ): row is {
          source: string;
          totalBlocks: number;
          availableBlocks: number;
          mountPoint: string;
        } => Boolean(row),
      );

    const fileSystems = await Promise.all(
      rows.map(async (row) => ({
        mountPoint: row.mountPoint,
        fileSystem: await getFilesystemType(row.mountPoint),
      })),
    );

    const fileSystemMap = new Map(
      fileSystems.map((entry) => [entry.mountPoint, entry.fileSystem] as const),
    );

    return rows
      .map((row) => {
        const mountPoint = row.mountPoint;
        const fsType = fileSystemMap.get(mountPoint) ?? "Unknown";
        const source = row.source;
        const kind = source.startsWith("/dev/")
          ? "Fixed"
          : source.startsWith("//") || source.startsWith("\\\\")
            ? "Network"
            : "Unknown";

        return {
          name: source,
          mount_point: mountPoint,
          file_system: fsType,
          total_bytes: row.totalBlocks * 1024,
          available_bytes: row.availableBlocks * 1024,
          kind,
          is_removable: false,
          model: null,
          media_type: null,
        } satisfies SystemDiskInfo;
      })
      .sort((a, b) => a.mount_point.localeCompare(b.mount_point));
  } catch {
    return [];
  }
}

export async function collectDiskInfos(): Promise<SystemDiskInfo[]> {
  if (process.platform === "win32") {
    return collectWindowsDisks();
  }

  return collectPosixDisks();
}

export async function testDiskSpeed(
  mountPoint: string,
  sizeBytes = TEST_FILE_SIZE,
): Promise<DiskSpeedResult> {
  if (!mountPoint.trim()) {
    throw new Error("Empty mount point");
  }

  const resolvedMount = path.resolve(mountPoint);
  const statInfo = await stat(resolvedMount).catch(() => null);
  if (!statInfo?.isDirectory()) {
    throw new Error("Invalid mount point");
  }

  const testDir = await mkdtemp(path.join(resolvedMount, "arrancador-speedtest-"));
  const testFile = path.join(testDir, "speedtest.bin");
  const buffer = Buffer.alloc(CHUNK_SIZE, 0xa5);
  const startedWrite = performance.now();

  try {
    const handle = await open(testFile, "w");
    try {
      let remaining = sizeBytes;
      while (remaining > 0) {
        const toWrite = Math.min(remaining, CHUNK_SIZE);
        await handle.write(buffer.subarray(0, toWrite));
        remaining -= toWrite;
      }
      await handle.sync();
    } finally {
      await handle.close();
    }

    const elapsedWriteMs = Math.max(1, Math.round(performance.now() - startedWrite));

    const startedRead = performance.now();
    const readBuffer = Buffer.alloc(CHUNK_SIZE);
    const reader = await open(testFile, "r");
    try {
      while (true) {
        const result = await reader.read(readBuffer, 0, readBuffer.length, null);
        if (result.bytesRead <= 0) {
          break;
        }
      }
    } finally {
      await reader.close();
    }

    const elapsedReadMs = Math.max(1, Math.round(performance.now() - startedRead));
    const sizeMb = sizeBytes / 1_048_576;

    return {
      mount_point: mountPoint,
      size_bytes: sizeBytes,
      write_mbps: sizeMb / (elapsedWriteMs / 1000),
      read_mbps: sizeMb / (elapsedReadMs / 1000),
      elapsed_write_ms: elapsedWriteMs,
      elapsed_read_ms: elapsedReadMs,
    };
  } finally {
    await rm(testDir, { recursive: true, force: true }).catch(() => undefined);
  }
}
