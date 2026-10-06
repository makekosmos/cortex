#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { documentHash, requireArgs } from "./release-utils.mjs";
import { verifyLocalReleaseManifest } from "./release-channel-local.mjs";
import { RELEASE_BOM_FILE } from "./release-bom.mjs";
import {
  legacyBom,
  legacyFeedFiles,
  MANIFEST_CREATOR_PLATFORM,
  manifestBytes,
  mergeReleaseManifest,
  parseReleaseManifest,
  RELEASE_MANIFEST_FILE,
} from "./release-manifest.mjs";
import { previousReleaseBaseline } from "./release-plan.mjs";
import { runReleasePreflight } from "./release-preflight.mjs";
import { releaseTarget } from "./release-repos.mjs";
import {
  assertExactArtifactSet,
  assertReceiptMatchesBom,
  readReceipt,
  verifyReceiptArtifacts,
} from "./release-receipt.mjs";
import { dmgName, installerName } from "./brand.mjs";

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
 * manifest.json already attached to release v<version>, or null when the
 * release has none yet. Fails closed on any gh/API error.
 */
export function fetchReleaseManifest(repository, version, run = spawnSync) {
  const options = {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
    env: process.env,
  };
  const view = run("gh", ["api", `repos/${repository}/releases/tags/v${version}`], options);
  if (view.error) throw view.error;
  if (view.status !== 0)
    die(`cannot read release v${version} assets: ${view.stderr?.trim() || "gh api failed"}`);
  const asset = (JSON.parse(view.stdout).assets ?? []).find(
    ({ name }) => name === RELEASE_MANIFEST_FILE,
  );
  if (!asset) return null;
  const raw = run(
    "gh",
    [
      "api",
      "-H",
      "Accept: application/octet-stream",
      `repos/${repository}/releases/assets/${asset.id}`,
    ],
    options,
  );
  if (raw.error) throw raw.error;
  if (raw.status !== 0)
    die(`cannot download ${RELEASE_MANIFEST_FILE} of v${version}: ${raw.stderr?.trim()}`);
  return parseReleaseManifest(raw.stdout);
}

/**
 * KOS-350 publish set: installer + manifest.json, plus — while
 * DUAL_PUBLISH_LEGACY_FEEDS is on — the legacy channel yml and (Windows only)
 * release-bom.v2.json, all rendered from that manifest. Provenance and
 * receipt are never uploaded. When the GitHub release already exists, the
 * bom is not re-uploaded so the release keeps a single bom file.
 * `manifestPath` is the merged manifest when the release already had one.
 */
export function publishAssetPaths({
  platform,
  version,
  outputDir,
  releaseAlreadyExists,
  manifestPath = path.join(outputDir, RELEASE_MANIFEST_FILE),
  dual,
}) {
  const installer = path.join(outputDir, installerFileName(platform, version));
  const legacy = legacyFeedFiles(platform, dual === undefined ? {} : { dual }).map((name) =>
    path.join(outputDir, name),
  );
  for (const file of [installer, manifestPath, ...legacy]) {
    if (!existsSync(file)) die(`missing publish artifact: ${file}`);
  }
  const uploads = releaseAlreadyExists
    ? legacy.filter((file) => path.basename(file) !== RELEASE_BOM_FILE)
    : legacy;
  return [installer, manifestPath, ...uploads];
}

// run(cmd, args) → { status, stdout, stderr } — injectable so tests never
// touch git. Default implementation spawns in the desktop directory.
function gitRun(cmd, args) {
  const result = spawnSync(cmd, args, {
    cwd: ROOT,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error) throw result.error;
  return { status: result.status ?? 1, stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

/**
 * GitHub release body: a header line plus every non-merge commit in
 * `fromCommit..toCommit` (exclusive of the previous release's source commit)
 * as `- <short hash> <subject> (<author>)`. Fails closed: a git error throws —
 * the release must never go out with no commit list.
 */
export function buildReleaseNotes({ version, fromCommit, toCommit, run = gitRun }) {
  const header = `Mundus ${version} installers + manifest.`;
  const result = run("git", [
    "log",
    "--no-merges",
    "--format=%h %s (%an)",
    `${fromCommit}..${toCommit}`,
  ]);
  if (result.status !== 0)
    die(
      `git log ${fromCommit}..${toCommit} failed (${result.status}): ${result.stderr?.trim() || "unknown git error"}`,
    );
  const commits = result.stdout
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const body = commits.length
    ? commits.map((line) => `- ${line}`).join("\n")
    : "No non-merge commits.";
  return `${header}\n\n${body}`;
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

  const { repo: repository } = releaseTarget(platform);
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
  // manifest.json is the source of truth: its entry must describe this
  // commit's installer, and the BOM it encodes must be the one derived from
  // HEAD. During dual-publish the legacy feeds are checked against it too.
  const localManifest = verifyLocalReleaseManifest(outputDir, version, platform);
  if (localManifest.platforms[platform].commit !== commit)
    die(`${RELEASE_MANIFEST_FILE} was built from another commit than HEAD`);
  if (legacyBom(localManifest, platform).digest !== bom.digest)
    die(`${RELEASE_MANIFEST_FILE} does not match the BOM derived from HEAD`);
  const legacyFiles = legacyFeedFiles(platform);
  if (
    legacyFiles.includes(RELEASE_BOM_FILE) &&
    documentHash(readFileSync(path.join(outputDir, RELEASE_BOM_FILE))) !== bom.digest
  )
    die("release BOM copy does not match the BOM derived from HEAD");

  // Local receipt lists installer + manifest (+ legacy feeds). Provenance /
  // receipt are not part of the published asset set (KOS-349).
  assertExactArtifactSet(receipt, [
    installerFileName(platform, version),
    RELEASE_MANIFEST_FILE,
    ...legacyFiles,
  ]);
  await verifyReceiptArtifacts(receipt, outputDir);

  const alreadyExists = dryRun ? false : releaseExists(repository, version);
  if (platform !== MANIFEST_CREATOR_PLATFORM && !alreadyExists && !dryRun)
    die(
      `${platform} publish requires cortex release v${version} to exist first (created by the Windows publish job)`,
    );
  let manifestPath;
  if (alreadyExists) {
    // Merge into the release's manifest so neither platform drops the
    // other's entry; macOS never rewrites the Windows-owned fields.
    const merged = mergeReleaseManifest(
      fetchReleaseManifest(repository, version),
      localManifest,
      platform,
    );
    const mergeDir = path.join(outputDir, "publish-manifest");
    mkdirSync(mergeDir, { recursive: true });
    manifestPath = path.join(mergeDir, RELEASE_MANIFEST_FILE);
    writeFileSync(manifestPath, manifestBytes(merged));
    console.log(
      `[publish-release] merged ${RELEASE_MANIFEST_FILE} platforms: ${Object.keys(merged.platforms).sort().join(", ")}`,
    );
  } else if (dryRun && platform !== MANIFEST_CREATOR_PLATFORM) {
    console.log(
      `[publish-release] dry run: would merge platforms.${platform} into the v${version} ${RELEASE_MANIFEST_FILE}`,
    );
  }
  const files = publishAssetPaths({
    platform,
    version,
    outputDir,
    releaseAlreadyExists: alreadyExists,
    manifestPath,
  });
  console.log(
    `[publish-release] verified local artifacts; uploading ${files.map((f) => path.basename(f)).join(", ")}`,
  );
  console.log(
    `[publish-release] plan: gh release ${alreadyExists ? "upload" : "create"} v${version} --repo ${repository}`,
  );

  // Release notes are built only on the Windows create path: the body lists
  // every non-merge commit since the previous published release's source
  // commit — the same baseline release-plan.mjs plans against, which on the
  // publish job is still the latest published stable release.
  let releaseNotes = null;
  if (platform === MANIFEST_CREATOR_PLATFORM && !alreadyExists) {
    if (dryRun) {
      // Dry-run has no GH_TOKEN: never call `gh` for the baseline.
      console.log(
        `[publish-release] dry run: would generate release notes for v${version} from previous published baseline`,
      );
    } else {
      const baseline = previousReleaseBaseline({ repo: repository });
      releaseNotes = buildReleaseNotes({ version, fromCommit: baseline.commit, toCommit: commit });
    }
  }

  if (dryRun) return;

  if (platform === "mac") {
    // Mac never creates the GitHub release or tag — that is Windows' job —
    // so the two cannot race on refs/tags/vX. The nightly mac job waits for
    // the release (and its manifest.json) to exist, then attaches the DMG and
    // the merged manifest (+ latest-mac.yml during dual-publish).
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
        releaseNotes,
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
