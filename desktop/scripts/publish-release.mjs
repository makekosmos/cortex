#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { documentHash, requireArgs } from "./release-utils.mjs";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";
import { RELEASE_BOM_FILE } from "./release-bom.mjs";
import { runReleasePreflight } from "./release-preflight.mjs";
import { releaseTarget } from "./release-repos.mjs";
import {
  assertExactArtifactSet,
  assertReceiptMatchesBom,
  readReceipt,
  verifyReceiptArtifacts,
} from "./release-receipt.mjs";
import { dmgName, installerName } from "./brand.mjs";
import { readFileSync } from "node:fs";

const ROOT = path.resolve(import.meta.dirname, "..");

function die(message) {
  throw new Error(message);
}

/** True when gh release view succeeds for v${version}. */
export function releaseExists(repository, version, run = spawnSync) {
  const result = run("gh", ["release", "view", `v${version}`, "--repo", repository], {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
    env: process.env,
  });
  if (result.error) throw result.error;
  return result.status === 0;
}

export function duplicateRelease(repository, version, run = spawnSync) {
  if (releaseExists(repository, version, run))
    die(`immutable release already exists: ${repository} v${version}`);
  // releaseExists already ran gh; re-probe for fail-closed auth errors when
  // the release is missing (status !== 0).
  const result = run("gh", ["release", "view", `v${version}`, "--repo", repository], {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
    env: process.env,
  });
  if (result.error) throw result.error;
  const output = `${result.stdout}\n${result.stderr}`.toLowerCase();
  if (!/(?:release\s+not\s+found|http\D*404|status\D*404)/i.test(output))
    die(`duplicate-release check failed closed: ${result.stderr?.trim() || "unknown gh error"}`);
}

function installerFileName(platform, version) {
  return platform === "mac" ? dmgName(version) : installerName(version);
}

/**
 * KOS-349 publish set: installer + channel yml + at most one bom.
 * Provenance and receipt are never uploaded.
 * When the GitHub release already exists (other platform published first),
 * skip re-uploading the bom so the release keeps a single bom file.
 */
export function publishAssetPaths({ platform, version, outputDir, releaseAlreadyExists }) {
  const { channelFile } = releaseTarget(platform);
  const installer = path.join(outputDir, installerFileName(platform, version));
  const channel = path.join(outputDir, channelFile);
  const bom = path.join(outputDir, RELEASE_BOM_FILE);
  for (const file of [installer, channel, bom]) {
    if (!existsSync(file)) die(`missing publish artifact: ${file}`);
  }
  if (releaseAlreadyExists) return [installer, channel];
  return [installer, channel, bom];
}

function ghRelease(args, dryRun) {
  console.log(`[publish-release] plan: gh ${args.join(" ")}`);
  if (dryRun) return;
  const result = spawnSync("gh", args, {
    cwd: ROOT,
    stdio: "inherit",
    windowsHide: true,
    env: process.env,
  });
  if (result.status !== 0) die(`gh ${args[0]} ${args[1]} failed`);
}

export async function main() {
  const dryRun = process.argv.includes("--dry-run");
  const local = process.argv.includes("--local") || process.env.MUNDUS_RELEASE_LOCAL === "1";
  const args = requireArgs(
    process.argv.filter((argument) => argument !== "--dry-run" && argument !== "--local"),
    ["platform", "receipt"],
  );
  const platform = args.platform;
  if (platform !== "win" && platform !== "mac") die(`Unknown platform "${platform}"`);
  if (args["also-bridge-repo"] !== undefined)
    die("--also-bridge-repo was removed: makekosmos/desktop is deleted (KOS-316)");

  const { repo: repository, channelFile } = releaseTarget(platform);
  const receiptPath = path.resolve(args.receipt);
  const receipt = await readReceipt(receiptPath);
  // --local / MUNDUS_RELEASE_LOCAL: mac nightly builds off plan.sha with a
  // locally bumped pin (dirty tree, detached HEAD). Win release on main
  // never sets this.
  const preflight = await runReleasePreflight({ platform, local });
  const { bom, currentCommit: commit, version } = preflight;
  if (!bom) die(`publish requires a BOM for platform ${platform}`);
  assertReceiptMatchesBom(receipt, {
    platform,
    version,
    currentCommit: commit,
    bom,
  });
  const outputDir = path.dirname(receiptPath);
  const bomCopy = path.join(outputDir, RELEASE_BOM_FILE);
  if (documentHash(readFileSync(bomCopy)) !== bom.digest)
    die("release BOM copy does not match the BOM derived from HEAD");
  verifyLocalReleaseChannel(outputDir, version, platform);

  // Local receipt lists installer + channel + bom. Provenance/receipt are not
  // part of the published asset set (KOS-349).
  assertExactArtifactSet(receipt, [
    installerFileName(platform, version),
    channelFile,
    RELEASE_BOM_FILE,
  ]);
  await verifyReceiptArtifacts(receipt, outputDir);

  const alreadyExists = dryRun ? false : releaseExists(repository, version);
  const files = publishAssetPaths({
    platform,
    version,
    outputDir,
    releaseAlreadyExists: alreadyExists,
  });
  console.log(
    `[publish-release] verified local artifacts; uploading ${files.map((f) => path.basename(f)).join(", ")}`,
  );
  console.log(
    `[publish-release] plan: gh release ${alreadyExists ? "upload" : "create"} v${version} --repo ${repository}`,
  );

  if (dryRun) return;

  if (platform === "mac") {
    // Mac never creates the GitHub release or tag — that is Windows' job —
    // so the two cannot race on refs/tags/vX. The nightly mac job waits for
    // the release to exist, then attaches DMG + latest-mac.yml.
    if (!alreadyExists)
      die(
        `macOS publish requires cortex release v${version} to exist first (created by the Windows publish job)`,
      );
    ghRelease(
      ["release", "upload", `v${version}`, ...files, "--repo", repository, "--clobber"],
      false,
    );
  } else if (alreadyExists) {
    // Retry after a partial publish (or a future mac-first create): attach
    // Windows assets without recreating the release.
    ghRelease(
      ["release", "upload", `v${version}`, ...files, "--repo", repository, "--clobber"],
      false,
    );
  } else {
    duplicateRelease(repository, version);
    ghRelease(
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
        "Mundus desktop release (installers + bom + updater channel).",
      ],
      false,
    );
  }

  const verify = spawnSync(
    process.execPath,
    [
      path.join(ROOT, "scripts", "verify-release-channel.mjs"),
      "--platform",
      platform,
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
