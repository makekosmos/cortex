import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { restoreBackupArtifact, restoreBackupDirectory } from "./restore";

describe("backup restore path safety", () => {
  it("restores a manifest backup through the TypeScript fallback", async () => {
    const previousBackend = process.env.ARRANCADOR_BACKUP_BACKEND;
    process.env.ARRANCADOR_BACKUP_BACKEND = "ts";
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-restore-test-"));
    const backup = path.join(root, "backup");
    const sourceDir = path.join(backup, "files", "root-0");
    const sourceFile = path.join(sourceDir, "save.dat");
    const restoreTarget = path.join(root, "restore", "save.dat");

    try {
      await mkdir(sourceDir, { recursive: true });
      await writeFile(sourceFile, "save-data", "utf8");
      await writeManifest(backup, "files/root-0/save.dat", restoreTarget);

      await restoreBackupDirectory(backup, [path.dirname(restoreTarget)]);

      await expect(readFile(restoreTarget, "utf8")).resolves.toBe("save-data");
    } finally {
      restoreBackend(previousBackend);
      await rm(root, { recursive: true, force: true });
    }
  });

  it("restores legacy mapping backups through the TypeScript fallback", async () => {
    const previousBackend = process.env.ARRANCADOR_BACKUP_BACKEND;
    process.env.ARRANCADOR_BACKUP_BACKEND = "ts";
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-restore-test-"));
    const backup = path.join(root, "legacy-backup");
    const restoreTarget = path.join(root, "legacy-restore", "save.dat");
    const targetMatch = restoreTarget.match(/^([A-Za-z]):[\\/](.*)$/);
    expect(targetMatch).toBeTruthy();
    const driveLabel = `drive-${targetMatch?.[1].toUpperCase()}`;
    const sourceFile = path.join(
      backup,
      driveLabel,
      (targetMatch?.[2] ?? "").replaceAll("\\", "/"),
    );
    const progress: string[] = [];

    try {
      await mkdir(path.dirname(sourceFile), { recursive: true });
      await writeFile(sourceFile, "legacy-save", "utf8");
      await writeFile(
        path.join(backup, "mapping.yaml"),
        [
          "drives:",
          `  ${driveLabel}: ${targetMatch?.[1].toUpperCase()}:`,
          "backups:",
          "  - name: legacy",
          "    files:",
          `      "${restoreTarget.replaceAll("\\", "/")}":`,
          "",
        ].join("\n"),
        "utf8",
      );

      await restoreBackupDirectory(backup, [path.dirname(restoreTarget)], (event) => {
        progress.push(`${event.stage}:${event.done}/${event.total}`);
      });

      await expect(readFile(restoreTarget, "utf8")).resolves.toBe("legacy-save");
      expect(progress).toEqual(["restore:1/1"]);
    } finally {
      restoreBackend(previousBackend);
      await rm(root, { recursive: true, force: true });
    }
  });

  it("rejects rooted, traversal, and drive-qualified manifest backup paths", async () => {
    const previousBackend = process.env.ARRANCADOR_BACKUP_BACKEND;
    process.env.ARRANCADOR_BACKUP_BACKEND = "ts";
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-restore-test-"));
    const backup = path.join(root, "backup");
    const restoreTarget = path.join(root, "restore", "save.dat");

    try {
      await mkdir(backup, { recursive: true });

      for (const backupPath of [
        "/files/root/save.dat",
        "\\\\server\\share\\save.dat",
        "C:/files/root/save.dat",
        "../files/root/save.dat",
        "files/root/../save.dat",
        "files/root//save.dat",
        "files/root/save.dat/",
      ]) {
        await writeManifest(backup, backupPath, restoreTarget);

        await expect(
          restoreBackupDirectory(backup, [path.dirname(restoreTarget)]),
        ).rejects.toThrow(
          "Invalid backup path in manifest",
        );
      }
    } finally {
      restoreBackend(previousBackend);
      await rm(root, { recursive: true, force: true });
    }
  });

  it("accepts slash-normalized allowed restore roots", async () => {
    const previousBackend = process.env.ARRANCADOR_BACKUP_BACKEND;
    process.env.ARRANCADOR_BACKUP_BACKEND = "ts";
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-restore-test-"));
    const backup = path.join(root, "backup");
    const sourceDir = path.join(backup, "files", "root-0");
    const restoreTarget = path.join(root, "allowed", "slot", "save.dat");

    try {
      await mkdir(sourceDir, { recursive: true });
      await writeFile(path.join(sourceDir, "save.dat"), "save-data", "utf8");
      await writeManifest(backup, "files/root-0/save.dat", restoreTarget);

      await restoreBackupDirectory(backup, [path.dirname(restoreTarget).replaceAll("\\", "/")]);

      await expect(readFile(restoreTarget, "utf8")).resolves.toBe("save-data");
    } finally {
      restoreBackend(previousBackend);
      await rm(root, { recursive: true, force: true });
    }
  });

  it("rejects unsafe manifest restore target paths", async () => {
    const previousBackend = process.env.ARRANCADOR_BACKUP_BACKEND;
    process.env.ARRANCADOR_BACKUP_BACKEND = "ts";
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-restore-test-"));
    const backup = path.join(root, "backup");

    try {
      await mkdir(backup, { recursive: true });

      for (const originalPath of [
        "relative/save.dat",
        "../save.dat",
        `${root}\\restore\\..\\save.dat`,
      ]) {
        await writeManifest(backup, "files/root-0/save.dat", originalPath);

        await expect(restoreBackupDirectory(backup, [root])).rejects.toThrow(
          "Invalid restore target path in manifest",
        );
      }
    } finally {
      restoreBackend(previousBackend);
      await rm(root, { recursive: true, force: true });
    }
  });

  it("rejects manifest restore targets outside allowed roots", async () => {
    const previousBackend = process.env.ARRANCADOR_BACKUP_BACKEND;
    process.env.ARRANCADOR_BACKUP_BACKEND = "ts";
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-restore-test-"));
    const backup = path.join(root, "backup");
    const sourceDir = path.join(backup, "files", "root-0");
    const outsideTarget = path.join(root, "outside", "save.dat");
    const allowedRoot = path.join(root, "allowed");

    try {
      await mkdir(sourceDir, { recursive: true });
      await writeFile(path.join(sourceDir, "save.dat"), "save-data", "utf8");
      await writeManifest(backup, "files/root-0/save.dat", outsideTarget);

      await expect(restoreBackupDirectory(backup, [allowedRoot])).rejects.toThrow(
        "Restore target is outside allowed roots",
      );
    } finally {
      restoreBackend(previousBackend);
      await rm(root, { recursive: true, force: true });
    }
  });

  it("rejects missing and unsupported backup artifacts", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-restore-test-"));
    const unsupported = path.join(root, "backup.txt");

    try {
      await writeFile(unsupported, "not a backup", "utf8");

      await expect(
        restoreBackupArtifact(path.join(root, "missing"), [root]),
      ).rejects.toThrow("Backup does not exist");
      await expect(restoreBackupArtifact(unsupported, [root])).rejects.toThrow(
        "Unsupported backup artifact",
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

async function writeManifest(
  backup: string,
  backupPath: string,
  originalPath: string,
) {
  await writeFile(
    path.join(backup, "__sqoba_manifest.json"),
    JSON.stringify({
      version: 2,
      files: [
        {
          backupPath,
          originalPath,
          size: 9,
          mtime: null,
        },
      ],
    }),
    "utf8",
  );
}

function restoreBackend(previousBackend: string | undefined) {
  if (previousBackend === undefined) {
    delete process.env.ARRANCADOR_BACKUP_BACKEND;
  } else {
    process.env.ARRANCADOR_BACKUP_BACKEND = previousBackend;
  }
}
