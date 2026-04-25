import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { buildBackupRelPath, copyDiscoveryToDirectory } from "./copy";
import type { SaveDiscovery } from "./types";

describe("backup copy path safety", () => {
  it("builds stable backup-relative paths for safe labels and relative paths", () => {
    expect(buildBackupRelPath("root-0", "slot/save.dat")).toBe(
      "files/root-0/slot/save.dat",
    );
    expect(
      buildBackupRelPath(
        "\u043a\u043e\u0440\u0435\u043d\u044c",
        "\u0441\u043b\u043e\u0442/\u4fdd\u5b58.dat",
      ),
    ).toBe(
      "files/\u043a\u043e\u0440\u0435\u043d\u044c/\u0441\u043b\u043e\u0442/\u4fdd\u5b58.dat",
    );
    expect(buildBackupRelPath("root-0", "")).toBe("files/root-0/file");
  });

  it("rejects traversal, absolute, drive-qualified, and unsafe root label paths", () => {
    expect(() => buildBackupRelPath("root-0", "../save.dat")).toThrow(
      "Invalid backup relative path segment",
    );
    expect(() => buildBackupRelPath("root-0", "slot/../save.dat")).toThrow(
      "Invalid backup relative path segment",
    );
    expect(() => buildBackupRelPath("root-0", "/absolute/save.dat")).toThrow(
      "Invalid backup relative path",
    );
    expect(() => buildBackupRelPath("root-0", "C:/Users/save.dat")).toThrow(
      "Invalid backup relative path",
    );
    expect(() => buildBackupRelPath("../root", "save.dat")).toThrow(
      "Invalid backup root label",
    );
  });

  it("copies discovery files into a constrained backup tree and writes a manifest", async () => {
    const previousBackend = process.env.ARRANCADOR_BACKUP_BACKEND;
    process.env.ARRANCADOR_BACKUP_BACKEND = "ts";
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-copy-test-"));
    const source = path.join(root, "source");
    const destination = path.join(root, "backup");
    const sourceFile = path.join(source, "save.dat");

    try {
      await mkdir(source, { recursive: true });
      await writeFile(sourceFile, "save-data");
      const discovery: SaveDiscovery = {
        roots: [{ label: "root-0", path: source }],
        files: [
          {
            path: sourceFile,
            rootLabel: "root-0",
            relativePath: "slot/save.dat",
            size: 9,
          },
        ],
        totalSize: 9,
      };

      const result = await copyDiscoveryToDirectory(destination, discovery, {
        concurrency: 1,
      });

      await expect(
        readFile(path.join(destination, "files", "root-0", "slot", "save.dat"), "utf8"),
      ).resolves.toBe("save-data");
      expect(result.totalBytes).toBe(9);
      expect(result.manifest.files).toEqual([
        expect.objectContaining({
          backupPath: "files/root-0/slot/save.dat",
          originalPath: sourceFile,
          size: 9,
        }),
      ]);
      await expect(
        readFile(path.join(destination, "__sqoba_manifest.json"), "utf8"),
      ).resolves.toContain("files/root-0/slot/save.dat");
    } finally {
      if (previousBackend === undefined) {
        delete process.env.ARRANCADOR_BACKUP_BACKEND;
      } else {
        process.env.ARRANCADOR_BACKUP_BACKEND = previousBackend;
      }
      await rm(root, { recursive: true, force: true });
    }
  });
});
