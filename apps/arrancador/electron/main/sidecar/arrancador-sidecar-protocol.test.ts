import { EventEmitter } from "node:events";
import { PassThrough } from "node:stream";
import { beforeEach, describe, expect, it, vi } from "vitest";

const spawnMock = vi.fn();

vi.mock("node:child_process", () => ({
  spawn: (...args: unknown[]) => spawnMock(...args),
}));

vi.mock("electron", () => ({
  app: { isPackaged: false },
}));

describe("managed sidecar protocol", () => {
  beforeEach(() => {
    vi.resetModules();
    spawnMock.mockReset();
  });

  it("drains progress events before handling process close", async () => {
    const child = createMockChild();
    spawnMock.mockReturnValue(child);
    const { scanExecutablesWithSidecar } = await import("./arrancador-sidecar");
    const entries: string[] = [];

    setTimeout(() => {
      child.stdout.write(
        `${JSON.stringify({
          event: "scan_entry",
          entry: { path: "C:\\Games\\Control.exe", file_name: "Control.exe" },
        })}\n`,
      );
      child.stdout.write(`${JSON.stringify({ ok: true, data: 1 })}\n`);
      child.stdout.end();
      child.emit("close", 0);
    }, 0);

    const count = await scanExecutablesWithSidecar("C:\\Games", {
      binaryPath: "mock sidecar.exe",
      onEntry: async (entry) => {
        await new Promise((resolve) => setTimeout(resolve, 50));
        entries.push(entry.file_name);
      },
    });

    expect(count).toBe(1);
    expect(entries).toEqual(["Control.exe"]);
    expect(spawnMock).toHaveBeenCalledWith("mock sidecar.exe", ["serve"], {
      stdio: ["pipe", "pipe", "pipe"],
      shell: false,
    });
  });

  it("routes backup progress events for copy and restore requests", async () => {
    const copyChild = createMockChild();
    const restoreChild = createMockChild();
    spawnMock.mockReturnValueOnce(copyChild).mockReturnValueOnce(restoreChild);
    const {
      copyBackupDirectoryWithSidecar,
      restoreBackupDirectoryWithSidecar,
    } = await import("./arrancador-sidecar");
    const progressMessages: string[] = [];

    queueSidecarLines(copyChild, [
      {
        event: "backup_progress",
        progress: { stage: "copy", current: "save.dat", done: 1, total: 1 },
      },
      { ok: true, data: { totalBytes: 9 } },
    ]);

    await expect(
      copyBackupDirectoryWithSidecar(
        "D:\\Backups",
        {
          roots: [{ label: "root-0", path: "D:\\Saves" }],
          files: [
            {
              path: "D:\\Saves\\save.dat",
              rootLabel: "root-0",
              relativePath: "save.dat",
              size: 9,
            },
          ],
          totalSize: 9,
        },
        {
          binaryPath: "mock-sidecar",
          onProgress: (progress) => {
            progressMessages.push(`${progress.stage}:${progress.current}`);
          },
        },
      ),
    ).resolves.toEqual({ totalBytes: 9 });

    queueSidecarLines(restoreChild, [
      {
        event: "backup_progress",
        progress: { stage: "restore", current: "save.dat", done: 1, total: 1 },
      },
      { ok: true, data: true },
    ]);

    await expect(
      restoreBackupDirectoryWithSidecar("D:\\Backups", {
        allowedRestoreRoots: ["D:\\Saves"],
        binaryPath: "mock-sidecar",
        onProgress: (progress) => {
          progressMessages.push(`${progress.stage}:${progress.current}`);
        },
      }),
    ).resolves.toBeUndefined();

    expect(progressMessages).toEqual(["copy:save.dat", "restore:save.dat"]);
  });

  it("rejects malformed and failed sidecar responses", async () => {
    const malformedChild = createMockChild();
    const failedChild = createMockChild();
    spawnMock.mockReturnValueOnce(malformedChild).mockReturnValueOnce(failedChild);
    const { scanExecutablesWithSidecar } = await import("./arrancador-sidecar");

    queueRawSidecarLines(malformedChild, ["not-json"]);
    await expect(
      scanExecutablesWithSidecar("C:\\Games", { binaryPath: "mock-sidecar" }),
    ).rejects.toBeInstanceOf(SyntaxError);

    queueSidecarLines(failedChild, [{ ok: false, error: "scan failed" }]);
    await expect(
      scanExecutablesWithSidecar("C:\\Games", { binaryPath: "mock-sidecar" }),
    ).rejects.toThrow("scan failed");
  });

  it("rejects stdin stream and write callback errors", async () => {
    const streamErrorChild = createMockChild();
    const writeErrorChild = createMockChild();
    writeErrorChild.stdin.write = vi.fn(
      (_chunk: unknown, callback?: (error?: Error | null) => void) => {
        callback?.(new Error("stdin write failed"));
        return true;
      },
    ) as never;
    spawnMock.mockReturnValueOnce(streamErrorChild).mockReturnValueOnce(writeErrorChild);
    const { scanExecutablesWithSidecar } = await import("./arrancador-sidecar");

    const streamErrorRequest = scanExecutablesWithSidecar("C:\\Games", {
      binaryPath: "mock-sidecar",
    });
    streamErrorChild.stdin.emit("error", new Error("stdin stream failed"));
    streamErrorChild.emit("close", 1);
    await expect(streamErrorRequest).rejects.toThrow("stdin stream failed");

    const writeErrorRequest = scanExecutablesWithSidecar("C:\\Games", {
      binaryPath: "mock-sidecar",
    });
    writeErrorChild.emit("close", 1);
    await expect(writeErrorRequest).rejects.toThrow("stdin write failed");
  });

  it("aborts managed requests and kills the child process", async () => {
    const child = createMockChild();
    spawnMock.mockReturnValue(child);
    const { scanExecutablesWithSidecar } = await import("./arrancador-sidecar");
    const controller = new AbortController();

    const request = scanExecutablesWithSidecar("C:\\Games", {
      binaryPath: "mock-sidecar",
      signal: controller.signal,
    });
    controller.abort();

    await expect(request).rejects.toThrow("Scan cancelled");
    expect(child.killed).toBe(true);
  });
});

function createMockChild() {
  const child = new EventEmitter() as EventEmitter & {
    stdin: PassThrough;
    stdout: PassThrough;
    stderr: PassThrough;
    killed: boolean;
    kill: () => void;
  };
  child.stdin = new PassThrough();
  child.stdout = new PassThrough();
  child.stderr = new PassThrough();
  child.killed = false;
  child.kill = () => {
    child.killed = true;
  };
  return child;
}

function queueSidecarLines(
  child: ReturnType<typeof createMockChild>,
  lines: readonly unknown[],
) {
  queueRawSidecarLines(
    child,
    lines.map((line) => JSON.stringify(line)),
  );
}

function queueRawSidecarLines(
  child: ReturnType<typeof createMockChild>,
  lines: readonly string[],
) {
  setTimeout(() => {
    for (const line of lines) {
      child.stdout.write(`${line}\n`);
    }
    child.stdout.end();
    child.emit("close", 0);
  }, 0);
}
