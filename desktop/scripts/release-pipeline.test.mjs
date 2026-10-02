import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scripts = path.join(import.meta.dirname);

test("release builds materialize runtime before preflight", async () => {
  const desktop = JSON.parse(await readFile(path.join(scripts, "..", "package.json"), "utf8"));
  const component = await readFile(path.join(scripts, "build-package-components.mjs"), "utf8");
  for (const script of [desktop.scripts.build, desktop.scripts["package:dir"]]) {
    assert.ok(script.indexOf("build:backend") < script.indexOf("release-preflight"));
  }
  // Swift helpers run on the release `build` chain and no-op off macOS.
  // package:dir stays the Windows staging recipe and does not grow a mac step.
  const build = desktop.scripts.build;
  const native = build.indexOf("build:native:macos");
  assert.ok(native > build.indexOf("release-preflight"));
  assert.ok(native < build.indexOf("build:package-components"));
  assert.equal(desktop.scripts["package:dir"].includes("build:native:macos"), false);
  // Preflight must run before any cargo component build — both markers
  // must actually be present for the comparison to mean anything.
  const pre = component.indexOf("release-preflight");
  const cargo = component.indexOf('"cargo"');
  assert.ok(pre >= 0 && cargo >= 0 && pre < cargo);
});

test("release build rejects a stale --bom flag before invoking makensis", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "mundus-preflight-"));
  const marker = path.join(dir, "builder-called");
  const result = spawnSync(
    process.execPath,
    [path.join(scripts, "build-desktop.mjs"), "--platform", "win", "--bom", "bom.json", "--local"],
    {
      cwd: path.join(scripts, ".."),
      encoding: "utf8",
      env: { ...process.env, MUNDUS_NSIS_DIR: marker },
    },
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Unknown argument "--bom"/);
  await assert.rejects(readFile(marker));
});

test("publish is a receipt consumer and never invokes build or package", async () => {
  const publish = (await readFile(path.join(scripts, "publish-release.mjs"), "utf8")).replaceAll(
    "\r\n",
    "\n",
  );
  assert.doesNotMatch(publish, /electron-builder|build-package-components|bun run build/);
  assert.match(publish, /verifyReceiptArtifacts/);
  assert.match(publish, /assertExactArtifactSet/);
  assert.match(publish, /assertReceiptMatchesBom/);
  assert.match(publish, /release create/);
  assert.ok(publish.indexOf("if (dryRun) return") < publish.lastIndexOf("duplicateRelease("));
  assert.ok(
    publish.indexOf("if (dryRun) return") < publish.indexOf('"release",\n        "create"'),
  );
  assert.ok(
    publish.indexOf('"release",\n        "create"') <
      publish.lastIndexOf("verify-release-channel.mjs"),
  );
});

// KOS-304: the primary target is always makekosmos/cortex; the legacy
// makekosmos/desktop feed is reachable only via the explicit bridge flag,
// which publishes the identical asset set to both repos.
test("publish targets: cortex only unless --also-bridge-repo opts in", async () => {
  const { publishTargets } = await import("./publish-release.mjs");
  assert.deepEqual(publishTargets({}), ["makekosmos/cortex"]);
  assert.deepEqual(publishTargets({ "also-bridge-repo": "makekosmos/desktop" }), [
    "makekosmos/cortex",
    "makekosmos/desktop",
  ]);
  assert.throws(() => publishTargets({ "also-bridge-repo": "not-a-repo" }), /owner\/repo/);
  assert.throws(
    () => publishTargets({ "also-bridge-repo": "makekosmos/cortex" }),
    /differ from the primary/,
  );
});

// KOS-304: both repos get the identical gh release create — same tag, files
// and notes — and the duplicate probe for every target runs before the first
// create, so a stale bridge release cannot orphan a cortex-only publish.
test("a bridge publish probes every repo before creating any", async () => {
  const publish = await readFile(path.join(scripts, "publish-release.mjs"), "utf8");
  const probes = publish.indexOf("for (const repository of repositories) duplicateRelease");
  const create = publish.indexOf('"release",\n        "create"');
  assert.ok(probes >= 0 && create > probes);
});

// The nightly job must authenticate with only its own GITHUB_TOKEN: no
// Actions secrets, no credential-helper PAT plumbing, and contents: write on
// the release job for the tag push and release creation.
test("the nightly workflow carries no PAT secrets", async () => {
  const workflow = await readFile(
    path.join(scripts, "..", "..", ".github", "workflows", "nightly-release.yml"),
    "utf8",
  );
  assert.doesNotMatch(workflow, /secrets\./);
  assert.doesNotMatch(workflow, /credential\.helper|persist-credentials:\s*false/);
  assert.match(workflow, /permissions:\s*\n\s*contents: write/);
  assert.match(workflow, /github\.token/);
});

test("duplicate release guard fails closed", async () => {
  const { duplicateRelease } = await import("./publish-release.mjs");
  const missing = () => ({ status: 1, stdout: "", stderr: "release not found" });
  assert.doesNotThrow(() => duplicateRelease("makekosmos/cortex", "1.2.3", missing));
  assert.throws(
    () => duplicateRelease("makekosmos/cortex", "1.2.3", () => ({ status: 0 })),
    /already exists/,
  );
  assert.throws(
    () =>
      duplicateRelease("makekosmos/cortex", "1.2.3", () => ({
        status: 1,
        stdout: "",
        stderr: "authentication failed",
      })),
    /failed closed/,
  );
});

test("receipt validation rejects mutation and stale inputs", async () => {
  const { createReceipt, verifyReceiptArtifacts, assertReceiptMatchesBom, normalizeArtifactPath } =
    await import("./release-receipt.mjs");
  const dir = await mkdtemp(path.join(os.tmpdir(), "mundus-receipt-"));
  const artifact = path.join(dir, "installer.exe");
  await writeFile(artifact, "good");
  const bom = {
    digest: "a".repeat(64),
    value: {
      id: "mundus-desktop-1.2.3-win",
      source: {
        toolchain: { pnpm: "1.0.0", node: "1.0.0", rust: "1.0.0", target: "x" },
      },
      compatibility: { engine_api: "1.0.0" },
    },
  };
  const current = { platform: "win", version: "1.2.3", currentCommit: "1".repeat(40), bom };
  const receipt = await createReceipt({ ...current, outputDir: dir, files: [artifact] });
  assert.equal((await verifyReceiptArtifacts(receipt, dir)).length, 1);
  assert.doesNotThrow(() => assertReceiptMatchesBom(receipt, current));
  for (const stale of [
    { ...current, version: "1.2.4" },
    { ...current, currentCommit: "2".repeat(40) },
    { ...current, bom: { ...bom, digest: "b".repeat(64) } },
  ])
    assert.throws(() => assertReceiptMatchesBom(receipt, stale), /stale/);
  receipt.inputs.pins.toolchain.pnpm = "9.9.9";
  assert.throws(() => assertReceiptMatchesBom(receipt, current), /stale/);
  receipt.inputs.pins.toolchain.pnpm = "1.0.0";
  await writeFile(artifact, "mutated");
  await assert.rejects(() => verifyReceiptArtifacts(receipt, dir), /hash|size|mutation/i);
  assert.throws(() => normalizeArtifactPath("./installer.exe"), /canonical/i);
  assert.throws(() => normalizeArtifactPath("installer/../installer.exe"), /canonical/i);
  assert.throws(() => normalizeArtifactPath("installer\\alias.exe"), /canonical/i);
});
