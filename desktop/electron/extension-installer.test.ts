import { expect, test } from "../test-support/node-test.mjs";
import { readFile } from "node:fs/promises";
import path from "node:path";

test("repoDevExtensionsRoot ignores packaged builds and resolves from repo root", async () => {
  const source = await readFile(path.join(import.meta.dirname, "extension-installer.ts"), "utf8");
  expect(source).toContain("if (app.isPackaged) return null;");
  expect(source).toContain('path.resolve(__dirname, "..", "..", "..", "extensions")');
});

test("resolveExtensionRootEntries keeps bundled extensions out of dev source", async () => {
  const source = await readFile(
    path.join(import.meta.dirname, "extension-package-registry.ts"),
    "utf8",
  );
  expect(source).toContain("if (!app.isPackaged) {");
  expect(source).toContain('const bundled = path.join(process.resourcesPath, "extensions");');
});
