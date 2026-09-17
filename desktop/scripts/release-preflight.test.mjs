import assert from "node:assert/strict";
import { test } from "node:test";
import { resolvePreflightArgs, runReleasePreflight } from "./release-preflight.mjs";

test("missing --bom falls back to KOSMOS_RELEASE_BOM", () => {
  const { platform, bomPath } = resolvePreflightArgs(["--platform", "win"], {
    KOSMOS_RELEASE_BOM: "bom.json",
  });
  assert.equal(platform, "win");
  assert.equal(bomPath, "bom.json");
});

test("explicit --bom wins over KOSMOS_RELEASE_BOM", () => {
  const { bomPath } = resolvePreflightArgs(["--platform", "win", "--bom", "explicit.json"], {
    KOSMOS_RELEASE_BOM: "env.json",
  });
  assert.equal(bomPath, "explicit.json");
});

test("a missing flag never resolves to another flag token", () => {
  // Regression: args[args.indexOf("--bom") + 1] used to return args[0] ("--platform"),
  // which made the env fallback unreachable and died with ENOENT on '--platform'.
  const { platform, bomPath } = resolvePreflightArgs(["--platform", "win"], {});
  assert.equal(platform, "win");
  assert.equal(bomPath, undefined);
});

test("missing BOM surfaces the required-BOM error instead of ENOENT", async () => {
  await assert.rejects(
    () => runReleasePreflight({ platform: "win", bomPath: undefined }),
    /--bom <path> or KOSMOS_RELEASE_BOM is required for release builds/,
  );
});

test("missing --platform is rejected as unknown platform", async () => {
  const { platform } = resolvePreflightArgs(["--bom", "bom.json"], {});
  await assert.rejects(
    () => runReleasePreflight({ platform, bomPath: "bom.json" }),
    /Unknown platform/,
  );
});
