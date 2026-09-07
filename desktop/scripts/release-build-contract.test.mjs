import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";

const script = await readFile(path.join(import.meta.dirname, "build-desktop.mjs"), "utf8");

test("release build validates before publishing immutable artifacts", () => {
  assert.match(script, /"--publish", "never"/);
  assert.ok(script.indexOf("emitProvenance(") < script.indexOf("publishRelease(platform"));
  assert.ok(
    script.indexOf("verifyLocalReleaseChannel(outputDir") <
      script.indexOf("publishRelease(platform"),
  );
  assert.ok(
    script.indexOf("emitProvenance(", script.indexOf("async function main")) <
      script.indexOf("runFirstPartyContracts(platform)"),
  );
  assert.ok(
    script.indexOf("runFirstPartyContracts(platform)") <
      script.indexOf("publishRelease(platform", script.indexOf("async function main")),
  );
  assert.doesNotMatch(script, /--clobber/);
  assert.match(script, /process\.env\.KOSMOS_RELEASE_BOM/);
  assert.match(script, /release builds require a clean tracked and source worktree/);
  assert.match(script, /ARK artifact hash does not match BOM/);
  assert.match(script, /app\.name\.endsWith\("\.app"\)/);
});

test("Windows installer carries the standalone engine inputs", () => {
  const packageJson = JSON.parse(
    readFileSync(path.join(import.meta.dirname, "../package.json"), "utf8"),
  );
  const resources = packageJson.build.win.extraResources;
  assert.ok(
    resources.some(
      ({ from, to }) => from === ".tmp/engine.next/Kosmos-Engine.zip" && to === "Kosmos Engine.zip",
    ),
  );
  assert.ok(
    resources.some(
      ({ from, to }) =>
        from === ".tmp/engine.next/engine-manifest.json" && to === "engine-manifest.json",
    ),
  );
});
