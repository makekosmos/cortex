import assert from "node:assert/strict";
import { test } from "node:test";
import {
  assertBuildingFromMain,
  assertVersionIsPublishable,
  resolvePreflightArgs,
  runReleasePreflight,
} from "./release-preflight.mjs";

test("missing --bom falls back to MUNDUS_RELEASE_BOM", () => {
  const { platform, bomPath } = resolvePreflightArgs(["--platform", "win"], {
    MUNDUS_RELEASE_BOM: "bom.json",
  });
  assert.equal(platform, "win");
  assert.equal(bomPath, "bom.json");
});

test("explicit --bom wins over MUNDUS_RELEASE_BOM", () => {
  const { bomPath } = resolvePreflightArgs(["--platform", "win", "--bom", "explicit.json"], {
    MUNDUS_RELEASE_BOM: "env.json",
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
    /--bom <path> or MUNDUS_RELEASE_BOM is required for release builds/,
  );
});

test("missing --platform is rejected as unknown platform", async () => {
  const { platform } = resolvePreflightArgs(["--bom", "bom.json"], {});
  await assert.rejects(
    () => runReleasePreflight({ platform, bomPath: "bom.json" }),
    /Unknown platform/,
  );
});

test("resolvePreflightArgs recognizes --local and MUNDUS_RELEASE_LOCAL (KOS-233)", () => {
  assert.equal(resolvePreflightArgs(["--platform", "win", "--local"], {}).local, true);
  assert.equal(
    resolvePreflightArgs(["--platform", "win"], { MUNDUS_RELEASE_LOCAL: "1" }).local,
    true,
  );
  assert.equal(resolvePreflightArgs(["--platform", "win"], {}).local, false);
});

test("assertBuildingFromMain rejects any branch other than main (KOS-233)", () => {
  assert.throws(
    () => assertBuildingFromMain("/repo", () => "kos-233-single-channel"),
    /must run from main HEAD \(current branch: kos-233-single-channel\)/,
  );
  assert.doesNotThrow(() => assertBuildingFromMain("/repo", () => "main"));
});

test("assertVersionIsPublishable requires a strictly newer version than the latest tag (KOS-233)", async () => {
  const fetchImpl = async () => ({ ok: true, json: async () => ({ tag_name: "v0.9.38" }) });
  await assert.rejects(
    () => assertVersionIsPublishable({ platform: "win", version: "0.9.38", fetchImpl }),
    /must be greater than the latest published 0\.9\.38/,
  );
  await assert.rejects(
    () => assertVersionIsPublishable({ platform: "win", version: "0.9.37", fetchImpl }),
    /must be greater than the latest published/,
  );
  await assert.doesNotReject(() =>
    assertVersionIsPublishable({ platform: "win", version: "0.9.39", fetchImpl }),
  );
});

test("assertVersionIsPublishable surfaces a clear error when the check is unreachable (KOS-233)", async () => {
  const fetchImpl = async () => ({ ok: false, status: 503 });
  await assert.rejects(
    () => assertVersionIsPublishable({ platform: "win", version: "9.9.9", fetchImpl }),
    /pass --local/,
  );
});
