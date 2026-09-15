import { access, lstat, mkdtemp, mkdir, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { expect, test } from "../test-support/node-test.mjs";
import { clearCrashReports, listCrashReports, resolveManagerDataDir } from "./diagnostics-files";

test("diagnostics files expose bounded regular-file metadata only", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "kosmos-manager-diagnostics-"));
  const crashes = path.join(root, "crashes");
  await mkdir(crashes);
  await writeFile(path.join(crashes, "safe.log"), "fixture");
  await writeFile(path.join(crashes, "ignore.txt"), "fixture");
  await mkdir(path.join(crashes, "nested.log"));
  const childLink = await symlink(path.join(crashes, "safe.log"), path.join(crashes, "link.log"))
    .then(() => true)
    .catch(() => false);
  const reports = await listCrashReports(root);
  expect(reports).toHaveLength(1);
  expect(reports[0]).toMatchObject({ name: "safe.log", size: 7 });
  expect(reports[0]).not.toHaveProperty("path");
  expect(reports[0].mtime.length).toBeLessThanOrEqual(64);
  await expect(clearCrashReports(root)).resolves.toBe(1);
  await expect(listCrashReports(root)).resolves.toEqual([]);
  if (childLink) await expect(access(path.join(crashes, "link.log"))).resolves.toBeUndefined();
});

test("missing roots are safe and data root follows the Engine override", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "kosmos-manager-empty-"));
  await expect(listCrashReports(path.join(root, "missing"))).resolves.toEqual([]);
  await expect(clearCrashReports(path.join(root, "missing"))).resolves.toBe(0);
  const previous = process.env.KOSMOS_DATA_DIR;
  process.env.KOSMOS_DATA_DIR = path.join(root, "override");
  expect(resolveManagerDataDir("C:\\Users\\fixture\\AppData")).toBe(process.env.KOSMOS_DATA_DIR);
  if (previous === undefined) delete process.env.KOSMOS_DATA_DIR;
  else process.env.KOSMOS_DATA_DIR = previous;
});

test("symlink roots are rejected without touching the target", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "kosmos-manager-link-root-"));
  const target = await mkdtemp(path.join(os.tmpdir(), "kosmos-manager-link-target-"));
  await mkdir(path.join(target, "crashes"));
  await writeFile(path.join(target, "crashes", "keep.log"), "keep");
  const linked = await symlink(path.join(target, "crashes"), path.join(root, "crashes"))
    .then(() => true)
    .catch(() => false);
  if (!linked) return;
  await expect(listCrashReports(root)).rejects.toThrow();
  await expect(clearCrashReports(root)).rejects.toThrow();
  await expect(access(path.join(target, "crashes", "keep.log"))).resolves.toBeUndefined();
  await expect(lstat(path.join(root, "crashes"))).resolves.toSatisfy((stat) =>
    stat.isSymbolicLink(),
  );
});
