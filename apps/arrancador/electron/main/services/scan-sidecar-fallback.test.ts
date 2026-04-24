import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

const sidecarMock = vi.hoisted(() => ({
  scanExecutablesWithSidecar: vi.fn(),
}));

vi.mock("../sidecar/arrancador-sidecar", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../sidecar/arrancador-sidecar")>();

  return {
    ...actual,
    scanExecutablesWithSidecar: sidecarMock.scanExecutablesWithSidecar,
  };
});

import { ArrancadorSidecarUnavailableError } from "../sidecar/arrancador-sidecar";
import { scanExecutablesStream } from "./scan";

const originalScanBackend = process.env.ARRANCADOR_SCAN_BACKEND;

afterEach(() => {
  sidecarMock.scanExecutablesWithSidecar.mockReset();
  vi.restoreAllMocks();

  if (originalScanBackend === undefined) {
    delete process.env.ARRANCADOR_SCAN_BACKEND;
  } else {
    process.env.ARRANCADOR_SCAN_BACKEND = originalScanBackend;
  }
});

describe("scanExecutablesStream", () => {
  it("falls back to the TypeScript scanner when the sidecar is unavailable", async () => {
    delete process.env.ARRANCADOR_SCAN_BACKEND;
    sidecarMock.scanExecutablesWithSidecar.mockRejectedValueOnce(
      new ArrancadorSidecarUnavailableError("missing sidecar"),
    );
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const root = await mkdtemp(path.join(tmpdir(), "arrancador-scan-"));

    try {
      await mkdir(path.join(root, "nested"));
      await mkdir(path.join(root, ".hidden"));
      await writeFile(path.join(root, "game.exe"), "");
      await writeFile(path.join(root, "readme.txt"), "");
      await writeFile(path.join(root, "nested", "tool.EXE"), "");
      await writeFile(path.join(root, ".hidden", "ignored.exe"), "");

      const entries: string[] = [];
      const count = await scanExecutablesStream(root, {
        onEntry: (entry) => {
          entries.push(entry.file_name);
        },
      });

      expect(count).toBe(2);
      expect(entries.sort()).toEqual(["game.exe", "tool.EXE"]);
      expect(sidecarMock.scanExecutablesWithSidecar).toHaveBeenCalledOnce();
      expect(warnSpy).toHaveBeenCalledWith(
        "[Arrancador] Rust scan sidecar unavailable, falling back to TypeScript scanner.",
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});
