import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";
import fs from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { app } from "electron";
import type { BackupProgress, SaveDiscovery } from "../services/backup/types";
import type { ExeEntry } from "../services/contracts";
import { createScanAbortError } from "../services/helpers/scan";

const MAX_STDERR_TAIL_CHARS = 32 * 1024;

interface SidecarResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

interface ScanEntryEvent {
  event: "scan_entry";
  entry: ExeEntry;
}

interface BackupProgressEvent {
  event: "backup_progress";
  progress: BackupProgress;
}

export interface ArrancadorSidecarPathOptions {
  appRoot?: string;
  isPackaged?: boolean;
  resourcesPath?: string;
}

export class ArrancadorSidecarUnavailableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ArrancadorSidecarUnavailableError";
  }
}

function appendTail(buffer: string, chunk: string, maxChars: number) {
  const next = buffer + chunk;
  if (next.length <= maxChars) {
    return next;
  }
  return next.slice(next.length - maxChars);
}

function getBinaryName() {
  return process.platform === "win32" ? "arrancador-sidecar.exe" : "arrancador-sidecar";
}

export function getArrancadorSidecarBinaryPath(
  options: ArrancadorSidecarPathOptions = {},
) {
  const binaryName = getBinaryName();
  const electronApp = app as typeof app | undefined;

  if (options.isPackaged ?? electronApp?.isPackaged ?? false) {
    return path.join(
      options.resourcesPath ?? process.resourcesPath,
      "arrancador-sidecar",
      binaryName,
    );
  }

  const appRoot = path.resolve(options.appRoot ?? process.env.APP_ROOT ?? process.cwd());
  const releasePath = path.join(appRoot, "sidecar", "target", "release", binaryName);
  if (fs.existsSync(releasePath)) {
    return releasePath;
  }

  return path.join(appRoot, "sidecar", "target", "debug", binaryName);
}

function isScanEntryEvent(value: unknown): value is ScanEntryEvent {
  return Boolean(
    value &&
      typeof value === "object" &&
      (value as { event?: unknown }).event === "scan_entry" &&
      typeof (value as { entry?: unknown }).entry === "object",
  );
}

function isBackupProgressEvent(value: unknown): value is BackupProgressEvent {
  return Boolean(
    value &&
      typeof value === "object" &&
      (value as { event?: unknown }).event === "backup_progress" &&
      typeof (value as { progress?: unknown }).progress === "object",
  );
}

function isUnavailableSpawnError(error: Error & { code?: string }) {
  return error.code === "ENOENT" || error.code === "EACCES" || error.code === "EPERM";
}

function createSidecarUnavailableError(binaryPath: string) {
  return new ArrancadorSidecarUnavailableError(
    `Arrancador sidecar is unavailable at ${binaryPath}`,
  );
}

function isUnavailableShellLaunchFailure(
  binaryPath: string,
  code: number | null,
  stderr: string,
) {
  if (process.platform !== "win32" || code === 0 || code === null) {
    return false;
  }

  if (!fs.existsSync(binaryPath)) {
    return true;
  }

  const lowerStderr = stderr.toLowerCase();
  const trimmedLowerStderr = lowerStderr.trim();
  const lowerBinaryPath = binaryPath.toLowerCase();
  const lowerBinaryName = path.basename(binaryPath).toLowerCase();
  const mentionsSidecarBinary =
    lowerStderr.includes(lowerBinaryPath) || lowerStderr.includes(lowerBinaryName);

  return (
    (mentionsSidecarBinary &&
      (lowerStderr.includes(
        "is not recognized as an internal or external command",
      ) ||
        lowerStderr.includes("the system cannot find the file specified") ||
        lowerStderr.includes("the system cannot find the path specified") ||
        lowerStderr.includes("access is denied"))) ||
    trimmedLowerStderr === "access is denied."
  );
}

export function isArrancadorSidecarUnavailableError(
  error: unknown,
): error is ArrancadorSidecarUnavailableError {
  return error instanceof ArrancadorSidecarUnavailableError;
}

async function runManagedSidecarRequest<T>(
  request: Record<string, unknown>,
  options: {
    signal?: AbortSignal;
    abortError?: () => Error;
    onEvent?: (event: unknown) => void | Promise<void>;
    binaryPath?: string;
  } = {},
): Promise<T> {
  if (options.signal?.aborted) {
    throw options.abortError?.() ?? createScanAbortError();
  }

  const binaryPath = options.binaryPath ?? getArrancadorSidecarBinaryPath();

  return await new Promise<T>((resolve, reject) => {
    let child: ChildProcessWithoutNullStreams | null = null;
    let stdoutBuffer = "";
    let stderrBuffer = "";
    let settled = false;
    let killedForAbort = false;

    const settle = (fn: () => void) => {
      if (settled) {
        return;
      }
      settled = true;
      options.signal?.removeEventListener("abort", abort);
      fn();
    };

    const abort = () => {
      killedForAbort = true;
      if (child && !child.killed) {
        child.kill();
      }
      settle(() => reject(options.abortError?.() ?? createScanAbortError()));
    };

    try {
      child = spawn(binaryPath, ["serve"], {
        stdio: ["pipe", "pipe", "pipe"],
        shell: process.platform === "win32",
      });
    } catch (error) {
      const spawnError = error instanceof Error ? error : new Error(String(error));
      const nextError = isUnavailableSpawnError(spawnError)
        ? createSidecarUnavailableError(binaryPath)
        : spawnError;
      settle(() => reject(nextError));
      return;
    }

    options.signal?.addEventListener("abort", abort, { once: true });

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");

    child.stderr.on("data", (chunk: string) => {
      stderrBuffer = appendTail(stderrBuffer, chunk, MAX_STDERR_TAIL_CHARS);
    });

    child.stdout.on("data", (chunk: string) => {
      stdoutBuffer += chunk;
      void flushStdout();
    });

    child.on("error", (error: Error & { code?: string }) => {
      const nextError = isUnavailableSpawnError(error)
        ? createSidecarUnavailableError(binaryPath)
        : error;
      settle(() => reject(nextError));
    });

    child.on("close", (code) => {
      if (killedForAbort || settled) {
        return;
      }
      if (isUnavailableShellLaunchFailure(binaryPath, code, stderrBuffer)) {
        settle(() => reject(createSidecarUnavailableError(binaryPath)));
        return;
      }
      const detail = stderrBuffer.trim() || `arrancador-sidecar exited with ${code}`;
      settle(() => reject(new Error(detail)));
    });

    child.stdin.write(`${JSON.stringify(request)}\n`);
    child.stdin.end();

    async function flushStdout() {
      while (true) {
        const newlineIndex = stdoutBuffer.indexOf("\n");
        if (newlineIndex === -1) {
          return;
        }

        const rawLine = stdoutBuffer.slice(0, newlineIndex).trim();
        stdoutBuffer = stdoutBuffer.slice(newlineIndex + 1);

        if (!rawLine) {
          continue;
        }

        let parsed: unknown;
        try {
          parsed = JSON.parse(rawLine) as SidecarResponse<T>;
        } catch (error) {
          settle(() =>
            reject(error instanceof Error ? error : new Error(String(error))),
          );
          return;
        }

        if (
          parsed &&
          typeof parsed === "object" &&
          "event" in parsed &&
          !("ok" in parsed)
        ) {
          await options.onEvent?.(parsed);
          continue;
        }

        const response = parsed as SidecarResponse<T>;
        if (!response.ok) {
          settle(() =>
            reject(new Error(response.error || "arrancador-sidecar request failed")),
          );
          return;
        }

        settle(() => resolve(response.data as T));
        return;
      }
    }
  });
}

export async function scanExecutablesWithSidecar(
  root: string,
  options: {
    signal?: AbortSignal;
    onEntry?: (entry: ExeEntry) => void | Promise<void>;
    binaryPath?: string;
  } = {},
): Promise<number> {
  const count = await runManagedSidecarRequest<number>(
    { operation: "scan_executables", root },
    {
      signal: options.signal,
      abortError: createScanAbortError,
      binaryPath: options.binaryPath,
      onEvent: async (event) => {
        if (isScanEntryEvent(event)) {
          await options.onEntry?.(event.entry);
        }
      },
    },
  );
  return Number(count ?? 0);
}

export interface CopyBackupDirectoryResult {
  totalBytes: number;
}

export async function copyBackupDirectoryWithSidecar(
  destination: string,
  discovery: SaveDiscovery,
  options: {
    onProgress?: (progress: BackupProgress) => void | Promise<void>;
    binaryPath?: string;
  } = {},
): Promise<CopyBackupDirectoryResult> {
  const root = await mkdtemp(path.join(tmpdir(), "arrancador-sidecar-copy-"));
  const requestPath = path.join(root, "request.jsonl");
  const lines = [
    JSON.stringify({ destination }),
    ...discovery.files.map((file) =>
      JSON.stringify({
        path: file.path,
        rootLabel: file.rootLabel,
        relativePath: file.relativePath,
        size: file.size,
      }),
    ),
  ];

  try {
    await writeFile(requestPath, `${lines.join("\n")}\n`, "utf8");
    return await runManagedSidecarRequest<CopyBackupDirectoryResult>(
      {
        operation: "copy_backup_directory",
        requestPath,
      },
      {
        binaryPath: options.binaryPath,
        onEvent: async (event) => {
          if (isBackupProgressEvent(event)) {
            await options.onProgress?.(event.progress);
          }
        },
      },
    );
  } finally {
    await rm(root, { recursive: true, force: true }).catch(() => undefined);
  }
}

export async function restoreBackupDirectoryWithSidecar(
  backupRoot: string,
  options: {
    onProgress?: (progress: BackupProgress) => void | Promise<void>;
    binaryPath?: string;
  } = {},
): Promise<void> {
  await runManagedSidecarRequest<boolean>(
    {
      operation: "restore_backup_directory",
      backupRoot,
    },
    {
      binaryPath: options.binaryPath,
      onEvent: async (event) => {
        if (isBackupProgressEvent(event)) {
          await options.onProgress?.(event.progress);
        }
      },
    },
  );
}
