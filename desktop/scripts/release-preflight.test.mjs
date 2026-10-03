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
  const fetchImpl = async () => ({ ok: true, json: async () => [{ tag_name: "v0.9.38" }] });
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

// KOS-304: a release repo that exists but has never published — cortex before
// the bridge release — has no baseline to beat, so any version is publishable.
// Only the list endpoint can tell this apart from a missing repo (which stays
// a hard error).
test("assertVersionIsPublishable passes when the repo has no releases yet", async () => {
  const fetchImpl = async (url) => {
    assert.equal(url, "https://api.github.com/repos/makekosmos/cortex/releases?per_page=100");
    return { ok: true, json: async () => [] };
  };
  await assert.doesNotReject(() =>
    assertVersionIsPublishable({ platform: "win", version: "0.10.1", fetchImpl }),
  );
});

test("assertVersionIsPublishable can check a bridge repository override", async () => {
  const seen = [];
  const fetchImpl = async (url) => {
    seen.push(url);
    return { ok: true, json: async () => [{ tag_name: "v0.10.0" }] };
  };
  await assert.rejects(
    () =>
      assertVersionIsPublishable({
        platform: "win",
        version: "0.10.0",
        repository: "makekosmos/desktop",
        fetchImpl,
      }),
    /must be greater than the latest published 0\.10\.0 on makekosmos\/desktop/,
  );
  assert.match(seen[0], /repos\/makekosmos\/desktop\//);
});

test("mac publishability is the desktop-mac repo and does not consult cortex", async () => {
  const seen = [];
  const fetchImpl = async (url) => {
    seen.push(url);
    return { ok: true, json: async () => [{ tag_name: "v0.5.1" }] };
  };
  await assert.rejects(
    () => assertVersionIsPublishable({ platform: "mac", version: "0.5.1", fetchImpl }),
    /must be greater than the latest published 0\.5\.1 on makekosmos\/desktop-mac/,
  );
  assert.deepEqual(seen, [
    "https://api.github.com/repos/makekosmos/desktop-mac/releases?per_page=100",
  ]);
});

test("mac preflight returns before the Windows engine check", async () => {
  const source = await readFile(path.join(import.meta.dirname, "release-preflight.mjs"), "utf8");
  const macReturn = source.indexOf('if (platform === "mac") return');
  const engine = source.indexOf("verifyEngineArtifact(version, commit);");
  assert.ok(macReturn > 0 && engine > macReturn);
});

test("assertVersionIsPublishable surfaces a clear error when the check is unreachable (KOS-233)", async () => {
  const fetchImpl = async () => ({ ok: false, status: 503 });
  await assert.rejects(
    () => assertVersionIsPublishable({ platform: "win", version: "9.9.9", fetchImpl }),
    /pass --local/,
  );
});
