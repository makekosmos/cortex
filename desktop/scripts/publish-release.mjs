#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { documentHash, requireArgs } from "./package-release-utils.mjs";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";
import { runReleasePreflight } from "./release-preflight.mjs";
import {
  assertExactArtifactSet,
  assertReceiptMatchesBom,
  readReceipt,
  verifyReceiptArtifacts,
} from "./release-receipt.mjs";

const ROOT = path.resolve(import.meta.dirname, "..");

function die(message) {
  throw new Error(message);
}

export function duplicateRelease(repository, version, run = spawnSync) {
  const result = run("gh", ["release", "view", `v${version}`, "--repo", repository], {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
    env: process.env,
  });
  if (result.error) throw result.error;
  if (result.status === 0) die(`immutable release already exists: ${repository} v${version}`);
  const output = `${result.stdout}\n${result.stderr}`.toLowerCase();
  if (!/(?:release\s+not\s+found|http\D*404|status\D*404)/i.test(output))
    die(`duplicate-release check failed closed: ${result.stderr?.trim() || "unknown gh error"}`);
}

export async function main() {
  const dryRun = process.argv.includes("--dry-run");
  const args = requireArgs(
    process.argv.filter((argument) => argument !== "--dry-run"),
    ["platform", "receipt"],
  );
  const platform = args.platform;
  if (!["win", "mac"].includes(platform)) die(`Unknown platform "${platform}"`);
  const receiptPath = path.resolve(args.receipt);
  const receipt = await readReceipt(receiptPath);
  const bomPath = receipt.inputs.bom.path;
  const preflight = await runReleasePreflight({ platform, bomPath });
  const { bom, currentCommit: commit, version } = preflight;
  assertReceiptMatchesBom(receipt, {
    platform,
    version,
    currentCommit: commit,
    bom,
  });

  const outputDir = path.dirname(receiptPath);
  const provenancePath = path.join(outputDir, "release-provenance.json");
  const provenance = JSON.parse(readFileSync(provenancePath, "utf8"));
  if (
    provenance.schema_version !== 1 ||
    provenance.platform !== platform ||
    provenance.version !== version ||
    provenance.bom_digest !== bom.digest ||
    !Array.isArray(provenance.artifacts)
  )
    die("release provenance does not match current publish inputs");
  const bomCopy = path.join(outputDir, "release-bom.v1.json");
  if (documentHash(readFileSync(bomCopy)) !== bom.digest)
    die("release BOM copy does not match the reviewed BOM");
  verifyLocalReleaseChannel(outputDir, platform, version);
  assertExactArtifactSet(receipt, [
    ...provenance.artifacts.map(({ name }) => name),
    "release-bom.v1.json",
    "release-provenance.json",
  ]);
  const artifacts = await verifyReceiptArtifacts(receipt, outputDir);
  const repository = platform === "win" ? "makekosmos/desktop" : "makekosmos/desktop-mac";
  const files = artifacts.map(({ file }) => file).concat(receiptPath);
  console.log(`[publish-release] verified ${artifacts.length} immutable artifacts`);
  console.log(`[publish-release] plan: gh release create v${version} --repo ${repository}`);
  if (dryRun) return;

  if (!process.env.GH_TOKEN) die("GH_TOKEN is required to publish a verified release");
  duplicateRelease(repository, version);
  const result = spawnSync(
    "gh",
    [
      "release",
      "create",
      `v${version}`,
      ...files,
      "--repo",
      repository,
      "--title",
      `Kosmos ${version}`,
      "--notes",
      "Immutable release assembled from the attached verification receipt.",
    ],
    { cwd: ROOT, stdio: "inherit", windowsHide: true, env: process.env },
  );
  if (result.status !== 0) die(`failed to publish verified release to ${repository}`);
  const verify = spawnSync(
    process.execPath,
    [
      path.join(ROOT, "scripts", "verify-release-channel.mjs"),
      "--platform",
      platform,
      "--version",
      version,
    ],
    { cwd: ROOT, stdio: "inherit", windowsHide: true },
  );
  if (verify.status !== 0) die(`verify-release-channel failed for ${platform} v${version}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(
      `[publish-release] FATAL: ${error instanceof Error ? error.message : String(error)}`,
    );
    process.exitCode = 1;
  });
}
