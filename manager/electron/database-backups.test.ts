import { lstat, mkdtemp, symlink } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { expect, test } from "bun:test";
import {
  normalizeDbBackupRestore,
  normalizeDbBackupValidation,
  normalizeDbBackups,
  resolveSafeDbBackupsFolder,
  validDbBackupId,
} from "./database-backups";

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

test("db backup id accepts only Core snapshot basenames", () => {
  expect(validDbBackupId("ark.db.backup-2026-09-16-153045")).toBe(true);
  for (const bad of [
    "",
    "ark.db",
    ".restore-rollback-1.db",
    "ark.db.backup-2026-09-16",
    "../ark.db.backup-2026-09-16-153045",
    "sub/ark.db.backup-2026-09-16-153045",
    "ark.db.backup-2026-09-16-153045.db",
    42,
    null,
  ])
    expect(validDbBackupId(bad)).toBe(false);
});

test("db backup list keeps only well-formed Core entries, newest first", () => {
  const backups = normalizeDbBackups({
    backups: [
      { id: "ark.db.backup-2026-09-15-100000", size_bytes: 1024, modified_ms: 1758000000000 },
      { id: "ark.db.backup-2026-09-16-153045", size_bytes: 2048 },
      { id: "notes.txt", size_bytes: 4 },
      { id: "../ark.db.backup-2026-09-16-153045", size_bytes: 4 },
      { id: "ark.db.backup-2026-09-16-153045", size_bytes: -1 },
      { id: "ark.db.backup-2026-09-16-153045", size_bytes: "2048" },
      { id: 7, size_bytes: 4 },
      "ark.db.backup-2026-09-16-153045",
    ],
  });
  expect(backups.map((b) => b.name)).toEqual([
    "ark.db.backup-2026-09-16-153045",
    "ark.db.backup-2026-09-15-100000",
  ]);
  expect(backups[0]?.size).toBe(2048);
  expect(backups[0]?.modified_at).toBeNull();
  expect(backups[1]?.modified_at).toBe(new Date(1758000000000).toISOString());
  expect(normalizeDbBackups(null)).toEqual([]);
  expect(normalizeDbBackups({ backups: "nope" })).toEqual([]);
});

test("db backup validation verdict is fail-closed on malformed responses", () => {
  const ok = normalizeDbBackupValidation({
    id: "ark.db.backup-2026-09-16-153045",
    exists: true,
    integrity_ok: true,
    schema_match: true,
    valid: true,
  });
  expect(ok.valid).toBe(true);
  expect(ok.error).toBeNull();

  const invalid = normalizeDbBackupValidation({
    id: "ark.db.backup-2026-09-16-153045",
    exists: true,
    integrity_ok: false,
    schema_match: false,
    valid: false,
    error: "integrity_check failed",
  });
  expect(invalid.valid).toBe(false);
  expect(invalid.error).toBe("integrity_check failed");

  // valid=true без подтверждающих флагов не принимается.
  expect(
    normalizeDbBackupValidation({
      id: "ark.db.backup-2026-09-16-153045",
      valid: true,
    }).valid,
  ).toBe(false);
  expect(normalizeDbBackupValidation(null).valid).toBe(false);
  expect(normalizeDbBackupValidation("x").valid).toBe(false);
});

test("db backup restore result requires restored flag and counts", () => {
  const ok = normalizeDbBackupRestore({
    id: "ark.db.backup-2026-09-16-153045",
    restored: true,
    objects: 12,
    links: 3,
  });
  expect(ok).toEqual({
    id: "ark.db.backup-2026-09-16-153045",
    restored: true,
    objects: 12,
    links: 3,
  });
  for (const bad of [
    null,
    "x",
    {},
    { id: "ark.db.backup-2026-09-16-153045", restored: false, objects: 1, links: 0 },
    { id: "../escape", restored: true, objects: 1, links: 0 },
    { id: "ark.db.backup-2026-09-16-153045", restored: true, objects: -1, links: 0 },
    { id: "ark.db.backup-2026-09-16-153045", restored: true, objects: 1 },
    { id: "ark.db.backup-2026-09-16-153045", restored: true, objects: 1.5, links: 0 },
  ])
    expect(normalizeDbBackupRestore(bad)).toBeNull();
});
