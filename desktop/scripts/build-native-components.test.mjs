import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { nativeBuildPlan } from "./build-native-components.mjs";
import { writeReleaseVersion } from "./release-version.mjs";

function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), "cortex-native-build-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(path.join(root, "desktop"));
  writeReleaseVersion("0.10.3", { root });
  return root;
}

test("host-native Engine and Manager use one real product version and source commit", (t) => {
  const root = fixture(t);
  const steps = nativeBuildPlan({
    root,
    sourceCommit: "abc123",
    inheritedEnv: { MUNDUS_PRODUCT_VERSION: "9.9.9" },
  });
  assert.equal(steps.length, 2);
  for (const step of steps) {
    assert.equal(step.env.MUNDUS_PRODUCT_VERSION, "0.10.3");
    assert.equal(step.env.MUNDUS_BUILD_CHANNEL, "stable");
    assert.equal(step.env.MUNDUS_ENGINE_SOURCE_COMMIT, "abc123");
    assert.ok(step.args.includes("--locked"));
    assert.ok(step.args.includes("--release"));
    assert.ok(!step.args.includes("--target"), "must not force the Windows target on other hosts");
  }
});

test("explicit dev build cannot accidentally inherit production metadata", (t) => {
  for (const step of nativeBuildPlan({
    root: fixture(t),
    dev: true,
    inheritedEnv: { MUNDUS_PRODUCT_VERSION: "0.10.3", MUNDUS_BUILD_CHANNEL: "stable" },
  })) {
    assert.equal(step.env.MUNDUS_PRODUCT_VERSION, "");
    assert.equal(step.env.MUNDUS_BUILD_CHANNEL, "dev");
  }
});
