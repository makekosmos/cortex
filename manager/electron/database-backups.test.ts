import { lstat, mkdtemp, symlink } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { expect, test } from "bun:test";
import { resolveSafeDbBackupsFolder } from "./database-backups";

test("backup folder validation rejects linked roots without touching the target", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "kosmos-manager-backup-root-"));
  const target = await mkdtemp(path.join(os.tmpdir(), "kosmos-manager-backup-target-"));
  const linked = await (
    process.platform === "win32"
      ? symlink(target, path.join(root, "backups"), "junction")
      : symlink(target, path.join(root, "backups"))
  )
    .then(() => true)
    .catch(() => false);
  if (!linked) return;

  await expect(resolveSafeDbBackupsFolder(root)).rejects.toThrow();
  expect((await lstat(target)).isDirectory()).toBe(true);
  expect((await lstat(path.join(root, "backups"))).isSymbolicLink()).toBe(true);
});
