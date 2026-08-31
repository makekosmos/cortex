import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";

const script = await readFile(
  path.join(import.meta.dirname, "first-party-release-contracts.mjs"),
  "utf8",
);
const desktopPackage = JSON.parse(
  await readFile(path.join(import.meta.dirname, "..", "package.json"), "utf8"),
);

test("first-party release contracts fail closed", () => {
  assert.match(script, /platform !== "win"/);
  assert.match(script, /\["run", "test:first-party-contracts"\]/);
  assert.match(script, /if \(result\.error\) throw result\.error/);
  assert.match(script, /if \(result\.status !== 0\)/);
  assert.equal(desktopPackage.scripts["package:mac"], "bun run build:mac");
});
