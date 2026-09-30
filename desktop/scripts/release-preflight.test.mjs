import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";
import {
  assertBuildingFromMain,
  assertVersionIsPublishable,
  resolvePreflightArgs,
  runReleasePreflight,
} from "./release-preflight.mjs";

test("resolvePreflightArgs reads --platform and never another flag token", () => {
  assert.equal(resolvePreflightArgs(["--platform", "win"], {}).platform, "win");
  assert.equal(resolvePreflightArgs(["--local"], {}).platform, undefined);
});

test("missing --platform is rejected as unknown platform", async () => {
  const { platform } = resolvePreflightArgs(["--local"], {});
  await assert.rejects(() => runReleasePreflight({ platform, local: true }), /Unknown platform/);
});

test("the release BOM is derived, never passed in", async () => {
  const source = await readFile(path.join(import.meta.dirname, "release-preflight.mjs"), "utf8");
  assert.match(source, /deriveReleaseBom\(/);
  assert.doesNotMatch(source, /--bom|RELEASE_BOM|bomPath/);
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
