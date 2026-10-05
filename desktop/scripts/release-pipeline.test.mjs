import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
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
  assert.match(publish, /"release",\n        "create"/);
  assert.ok(publish.indexOf("if (dryRun) return") < publish.lastIndexOf("duplicateRelease("));
  const createAt = publish.indexOf('"release",\n        "create"');
  assert.ok(publish.indexOf("if (dryRun) return") < createAt);
  assert.ok(createAt < publish.lastIndexOf("verify-release-channel.mjs"));
});

// The only publish target is makekosmos/cortex; the KOS-304 bridge flag is
// gone and must fail fast rather than silently publish cortex-only.
test("publish targets: the removed --also-bridge-repo flag dies", async () => {
  const publish = await readFile(path.join(scripts, "publish-release.mjs"), "utf8");
  assert.match(publish, /also-bridge-repo.*removed/s);
});

// The duplicate-release probe runs before the release create so an existing
// tag fails before anything is published.
test("the duplicate-release probe runs before the create", async () => {
  const publish = await readFile(path.join(scripts, "publish-release.mjs"), "utf8");
  const probes = publish.indexOf("duplicateRelease(repository, version)");
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

test("publish uploads installer + manifest.json and never provenance/receipt (KOS-350)", async () => {
  const publish = await readFile(path.join(scripts, "publish-release.mjs"), "utf8");
  assert.match(publish, /publishAssetPaths/);
  assert.doesNotMatch(publish, /release-provenance\.json/);
  assert.match(publish, /RELEASE_MANIFEST_FILE/);
  assert.match(publish, /mergeReleaseManifest/);
  assert.match(publish, /legacyFeedFiles/);
  assert.match(publish, /"release", "upload"/);
});

async function publishDir(platform, names) {
  const dir = await mkdtemp(path.join(os.tmpdir(), `mundus-publish-${platform}-`));
  for (const name of names) await writeFile(path.join(dir, name), name);
  return dir;
}

test("publish set: dual-publish adds the legacy feeds; cutover is installer + manifest", async () => {
  const { publishAssetPaths } = await import("./publish-release.mjs");
  const win = await publishDir("win", [
    "Mundus-Setup-1.2.3.exe",
    "manifest.json",
    "latest.yml",
    "release-bom.v2.json",
  ]);
  const names = (files) => files.map((file) => path.basename(file));
  const args = { platform: "win", version: "1.2.3", outputDir: win };
  assert.deepEqual(names(publishAssetPaths({ ...args, releaseAlreadyExists: false })), [
    "Mundus-Setup-1.2.3.exe",
    "manifest.json",
    "latest.yml",
    "release-bom.v2.json",
  ]);
  // Existing release (retry): keep the single bom already attached.
  assert.deepEqual(names(publishAssetPaths({ ...args, releaseAlreadyExists: true })), [
    "Mundus-Setup-1.2.3.exe",
    "manifest.json",
    "latest.yml",
  ]);
  assert.deepEqual(
    names(publishAssetPaths({ ...args, releaseAlreadyExists: false, dual: false })),
    ["Mundus-Setup-1.2.3.exe", "manifest.json"],
  );
  const mac = await publishDir("mac", ["Mundus-1.2.3.dmg", "manifest.json", "latest-mac.yml"]);
  const merged = path.join(mac, "publish-manifest", "manifest.json");
  await mkdir(path.dirname(merged));
  await writeFile(merged, "{}");
  const macArgs = { platform: "mac", version: "1.2.3", outputDir: mac };
  const macFiles = publishAssetPaths({
    ...macArgs,
    releaseAlreadyExists: true,
    manifestPath: merged,
  });
  assert.deepEqual(names(macFiles), ["Mundus-1.2.3.dmg", "manifest.json", "latest-mac.yml"]);
  assert.equal(macFiles[1], merged);
  assert.deepEqual(
    names(publishAssetPaths({ ...macArgs, releaseAlreadyExists: true, dual: false })),
    ["Mundus-1.2.3.dmg", "manifest.json"],
  );
  await rm(win, { recursive: true, force: true });
  await rm(path.join(mac, "latest-mac.yml"));
  assert.throws(
    () => publishAssetPaths({ ...macArgs, releaseAlreadyExists: true }),
    /missing publish artifact/,
  );
  await rm(mac, { recursive: true, force: true });
});

test("fetchReleaseManifest returns null without the asset and fails closed on gh errors", async () => {
  const { fetchReleaseManifest } = await import("./publish-release.mjs");
  const view = (assets) => ({ status: 0, stdout: JSON.stringify({ assets }), stderr: "" });
  assert.equal(
    fetchReleaseManifest("makekosmos/cortex", "1.2.3", () => view([{ name: "latest.yml", id: 1 }])),
    null,
  );
  assert.throws(
    () =>
      fetchReleaseManifest("makekosmos/cortex", "1.2.3", () => ({
        status: 1,
        stdout: "",
        stderr: "HTTP 500",
      })),
    /cannot read release/,
  );
  const calls = [];
  const run = (cmd, args) => {
    calls.push(args.join(" "));
    if (args.includes("Accept: application/octet-stream"))
      return { status: 0, stdout: "{}", stderr: "" };
    return view([{ name: "manifest.json", id: 42 }]);
  };
  assert.throws(() => fetchReleaseManifest("makekosmos/cortex", "1.2.3", run), /schema/);
  assert.ok(calls.some((line) => line.endsWith("releases/assets/42")));
});

test("RELEASE_REPOS.mac points at cortex, not desktop-mac (KOS-349)", async () => {
  const { RELEASE_REPOS, releaseTarget } = await import("./release-repos.mjs");
  assert.equal(RELEASE_REPOS.win, "makekosmos/cortex");
  assert.equal(RELEASE_REPOS.mac, "makekosmos/cortex");
  assert.equal(releaseTarget("mac").channelFile, "latest-mac.yml");
  assert.equal(releaseTarget("mac").manifestFile, "manifest.json");
  assert.notEqual(RELEASE_REPOS.mac, "makekosmos/desktop-mac");
});

test("nightly mac job does not gate the Windows release job (KOS-349)", async () => {
  const workflow = await readFile(
    path.join(scripts, "..", "..", ".github", "workflows", "nightly-release.yml"),
    "utf8",
  );
  assert.match(workflow, /release-mac:/);
  assert.match(workflow, /needs: \[plan\]/);
  // Windows release must not need release-mac.
  const winBlock = workflow.split("release-mac:")[0];
  assert.match(winBlock, /needs: \[plan, smoke\]/);
  assert.doesNotMatch(winBlock, /release-mac/);
});

test("nightly mac publish waits for the Windows-created manifest.json (KOS-350)", async () => {
  const workflow = await readFile(
    path.join(scripts, "..", "..", ".github", "workflows", "nightly-release.yml"),
    "utf8",
  );
  const macBlock = workflow.split("release-mac:")[1];
  assert.match(macBlock, /grep -qx 'manifest\.json'/);
});
