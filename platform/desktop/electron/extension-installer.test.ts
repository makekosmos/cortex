import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import path from "node:path";

test("repoDevExtensionsRoot ignores packaged builds and resolves from repo root", async () => {
  const source = await readFile(path.join(import.meta.dir, "extension-installer.ts"), "utf8");
  expect(source).toContain("if (app.isPackaged) return null;");
  expect(source).toContain('path.resolve(__dirname, "..", "..", "..", "extensions")');
});

test("resolveExtensionRootEntries keeps bundled extensions out of dev source", async () => {
  const source = await readFile(path.join(import.meta.dir, "extension-host.ts"), "utf8");
  expect(source).toContain("if (!app.isPackaged) {");
  expect(source).toContain('const bundled = path.join(process.resourcesPath, "extensions");');
});
