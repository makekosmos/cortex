import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  checkBackupNeeded,
  checkRestoreNeeded,
  createBackup,
  deleteBackup,
  listBackups,
  restoreBackup,
} from "./index";

describe("backup service facade", () => {
  it("creates, lists, restores, and deletes directory backups", async () => {
    const previousBackend = process.env.ARRANCADOR_BACKUP_BACKEND;
    process.env.ARRANCADOR_BACKUP_BACKEND = "ts";
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-backup-facade-"));
    const saves = path.join(root, "saves");
    const backupRoot = path.join(root, "backups");
    const saveFile = path.join(saves, "slot1.sav");
    const progress: string[] = [];

    try {
      await mkdir(saves, { recursive: true });
      await writeFile(saveFile, "save-data", "utf8");

      const backup = await createBackup({
        gameId: "game-1",
        gameName: "Arcadia",
        backupRoot,
        mode: "directory",
        overridePath: saves,
        isAuto: false,
        gameYear: "2026",
        onProgress: (event) => {
          progress.push(event.stage);
        },
      });

      expect(backup.backupSize).toBe(9);
      expect(progress).toContain("scan");
      expect(progress).toContain("copy");
      expect(progress).toContain("done");
      await expect(
        listBackups({ backupRoot, gameName: "Arcadia", gameYear: "2026" }),
      ).resolves.toEqual([
        expect.objectContaining({
          backupPath: backup.backupPath,
          backupSize: 9,
          kind: "directory",
        }),
      ]);

      await writeFile(saveFile, "changed", "utf8");
      await restoreBackup({
        backupPath: backup.backupPath,
        allowedRestoreRoots: [saves],
      });
      await expect(readFile(saveFile, "utf8")).resolves.toBe("save-data");

      await deleteBackup({ backupPath: backup.backupPath });
      await expect(
        listBackups({ backupRoot, gameName: "Arcadia", gameYear: "2026" }),
      ).resolves.toEqual([]);
    } finally {
      if (previousBackend === undefined) {
        delete process.env.ARRANCADOR_BACKUP_BACKEND;
      } else {
        process.env.ARRANCADOR_BACKUP_BACKEND = previousBackend;
      }
      await rm(root, { recursive: true, force: true });
    }
  });

  it("evaluates backup and restore need from current and previous sizes", async () => {
    const currentSave = {
      roots: [{ label: "root-0", path: "C:\\Saves" }],
      files: [],
      totalSize: 10,
    };

    await expect(
      checkBackupNeeded({ currentSave: null, lastBackup: null }),
    ).resolves.toBe(false);
    await expect(
      checkBackupNeeded({ currentSave, lastBackup: null }),
    ).resolves.toBe(true);
    await expect(
      checkRestoreNeeded({
        currentSave,
        lastBackup: {
          id: "backup-1",
          backupPath: "C:\\Backups\\backup-1",
          backupSize: 20,
          createdAt: "2026-04-24T10:00:00.000Z",
        },
      }),
    ).resolves.toEqual({
      shouldRestore: true,
      backupId: "backup-1",
      currentSize: 10,
      backupSize: 20,
    });
  });
});
