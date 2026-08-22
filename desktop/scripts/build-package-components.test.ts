import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";

test("packaged Manager and Host inherit the Desktop release version", () => {
  const source = readFileSync(path.join(import.meta.dir, "build-package-components.mjs"), "utf8");
  expect(source).toContain('release-versions.json"), "utf8"');
  expect(source).toContain("--config.extraMetadata.version=${version}");
});
