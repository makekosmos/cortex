import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import path from "node:path";

const source = await readFile(
  path.join(import.meta.dir, "../src/views/settings/composables/useExtensionsTab.ts"),
  "utf8",
);

test("settings use Runtime packages rather than the retired extension marketplace", () => {
  expect(source).toContain('"packages.list"');
  expect(source).toContain('"packages.refresh_catalog"');
  expect(source).toContain('"packages.install"');
  expect(source).toContain('"packages.set_enabled"');
  expect(source).toContain("version: item.version");
  expect(source).not.toContain("window.kepler.extension");
});
