#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { documentHash, requireArgs } from "./release-utils.mjs";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";
import { RELEASE_BOM_FILE } from "./release-bom.mjs";
import { runReleasePreflight } from "./release-preflight.mjs";
import { RELEASE_REPOS } from "./release-repos.mjs";
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
  if (platform !== "win") die(`Unknown platform "${platform}"`);
  if (args["also-bridge-repo"] !== undefined)
    die("--also-bridge-repo was removed: makekosmos/desktop is deleted (KOS-316)");
  const repository = RELEASE_REPOS.win;
  const receiptPath = path.resolve(args.receipt);
  const receipt = await readReceipt(receiptPath);
  const preflight = await runReleasePreflight({ platform });
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
  const bomCopy = path.join(outputDir, RELEASE_BOM_FILE);
  if (documentHash(readFileSync(bomCopy)) !== bom.digest)
    die("release BOM copy does not match the BOM derived from HEAD");
  verifyLocalReleaseChannel(outputDir, version);
  assertExactArtifactSet(receipt, [
    ...provenance.artifacts.map(({ name }) => name),
    RELEASE_BOM_FILE,
    "release-provenance.json",
  ]);
  const artifacts = await verifyReceiptArtifacts(receipt, outputDir);
  const files = artifacts.map(({ file }) => file).concat(receiptPath);
  console.log(`[publish-release] verified ${artifacts.length} immutable artifacts`);
  console.log(`[publish-release] plan: gh release create v${version} --repo ${repository}`);
  if (dryRun) return;

  // gh brings its own auth (keyring or GH_TOKEN); the duplicate-release probe
  // fails closed on any auth error before anything is created.
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
      `Mundus ${version}`,
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
      "--version",
      version,
      "--repo",
      repository,
    ],
    { cwd: ROOT, stdio: "inherit", windowsHide: true },
  );
  if (verify.status !== 0) die(`verify-release-channel failed for ${repository} v${version}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(
      `[publish-release] FATAL: ${error instanceof Error ? error.message : String(error)}`,
    );
    process.exitCode = 1;
  });
}
