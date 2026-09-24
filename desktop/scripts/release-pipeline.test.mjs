import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scripts = path.join(import.meta.dirname);

test("all local electron-builder package paths explicitly disable publishing", async () => {
  const desktop = JSON.parse(await readFile(path.join(scripts, "..", "package.json"), "utf8"));
  const component = await readFile(path.join(scripts, "build-package-components.mjs"), "utf8");
  assert.match(desktop.scripts["package:dir"], /--publish never/);
  assert.match(component, /"--publish",\s*"never"/);
  const releaseBuild = await readFile(path.join(scripts, "build-desktop.mjs"), "utf8");
  assert.doesNotMatch(releaseBuild, /gh\s+release\s+(?:view|create)/);
  assert.match(releaseBuild, /"--publish",\s*"never"/);
});

test("release builds materialize runtime before preflight", async () => {
  const desktop = JSON.parse(await readFile(path.join(scripts, "..", "package.json"), "utf8"));
  const component = await readFile(path.join(scripts, "build-package-components.mjs"), "utf8");
  for (const script of [
    desktop.scripts.build,
    desktop.scripts["build:mac"],
    desktop.scripts["package:dir"],
  ]) {
    assert.ok(script.indexOf("build:backend") < script.indexOf("release-preflight"));
  }
  assert.ok(component.indexOf("release-preflight") < component.indexOf("const version"));
});

test("macOS release packaging performs final preflight", async () => {
  const desktop = JSON.parse(await readFile(path.join(scripts, "..", "package.json"), "utf8"));
  assert.doesNotMatch(
    desktop.scripts["build:mac"],
    /build-desktop\.mjs --platform mac --skip-preflight/,
  );
});

test("release build rejects a missing BOM before invoking electron-builder", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "kosmos-preflight-"));
  const marker = path.join(dir, "builder-called");
  const builder = path.join(dir, "builder.mjs");
  await writeFile(
    builder,
    `import { writeFileSync } from "node:fs"; writeFileSync(${JSON.stringify(marker)}, "called");`,
  );
  const result = spawnSync(
    process.execPath,
    [
      path.join(scripts, "build-desktop.mjs"),
      "--platform",
      "win",
      "--bom",
      path.join(dir, "missing.json"),
    ],
    {
      cwd: path.join(scripts, ".."),
      encoding: "utf8",
      env: { ...process.env, KOSMOS_ELECTRON_BUILDER: `${process.execPath} ${builder}` },
    },
  );
  assert.notEqual(result.status, 0);
  assert.match(`${result.stdout}\n${result.stderr}`, /ENOENT|missing\.json|BOM|clean tracked/i);
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
  assert.ok(publish.indexOf("if (dryRun) return") < publish.indexOf('"release",\n      "create"'));
  assert.ok(
    publish.indexOf('"release",\n      "create"') <
      publish.lastIndexOf("verify-release-channel.mjs"),
  );
});

test("duplicate release guard fails closed", async () => {
  const { duplicateRelease } = await import("./publish-release.mjs");
  const missing = () => ({ status: 1, stdout: "", stderr: "release not found" });
  assert.doesNotThrow(() => duplicateRelease("makekosmos/desktop", "1.2.3", missing));
  assert.throws(
    () => duplicateRelease("makekosmos/desktop", "1.2.3", () => ({ status: 0 })),
    /already exists/,
  );
  assert.throws(
    () =>
      duplicateRelease("makekosmos/desktop", "1.2.3", () => ({
        status: 1,
        stdout: "",
        stderr: "authentication failed",
      })),
    /failed closed/,
  );
});

test("receipt validation rejects mutation and stale inputs", async () => {
  const {
    createReceipt,
    verifyReceiptArtifacts,
    assertReceiptInputs,
    assertReceiptMatchesBom,
    normalizeArtifactPath,
  } = await import("./release-receipt.mjs");
  const dir = await mkdtemp(path.join(os.tmpdir(), "kosmos-receipt-"));
  const artifact = path.join(dir, "installer.exe");
  await writeFile(artifact, "good");
  const bom = {
    path: path.join(dir, "bom.json"),
    digest: "a".repeat(64),
    value: {
      id: "release-bom",
      source: {
        cortex: { commit: "1".repeat(40) },
        core: { commit: "2".repeat(40) },
        arca_sdk: { commit: "3".repeat(40) },
        imago: { commit: "4".repeat(40) },
        store: { commit: "5".repeat(40) },
        toolchain: { bun: "1.0.0", node: "1.0.0", rust: "1.0.0" },
      },
      compatibility: { shell_api: "1.0.0", engine_api: "1.0.0", package_schema: 2 },
      catalog: { sequence: 1 },
    },
  };
  const receipt = await createReceipt({
    outputDir: dir,
    platform: "win",
    version: "1.2.3",
    currentCommit: "1".repeat(40),
    bom,
    files: [artifact],
  });
  assert.equal((await verifyReceiptArtifacts(receipt, dir)).length, 1);
  assert.doesNotThrow(() =>
    assertReceiptMatchesBom(receipt, {
      platform: "win",
      version: "1.2.3",
      currentCommit: "1".repeat(40),
      bom,
    }),
  );
  receipt.inputs.pins.toolchain.bun = "9.9.9";
  assert.throws(
    () =>
      assertReceiptMatchesBom(receipt, {
        platform: "win",
        version: "1.2.3",
        currentCommit: "1".repeat(40),
        bom,
      }),
    /pins|stale/i,
  );
  receipt.inputs.pins.toolchain.bun = "1.0.0";
  await writeFile(artifact, "mutated");
  await assert.rejects(() => verifyReceiptArtifacts(receipt, dir), /hash|size|mutation/i);
  assert.throws(
    () => assertReceiptInputs(receipt, { platform: "win", version: "1.2.4" }),
    /version/i,
  );
  assert.throws(() => normalizeArtifactPath("./installer.exe"), /canonical/i);
  assert.throws(() => normalizeArtifactPath("installer/../installer.exe"), /canonical/i);
  assert.throws(() => normalizeArtifactPath("installer\\alias.exe"), /canonical/i);
});
