#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { getVersion } from "./release-version.mjs";
import { deriveReleaseBom } from "./release-bom.mjs";

export const SHELL_ROOT = path.resolve(import.meta.dirname, "..");

export function currentCommit() {
  return execFileSync("git", ["rev-parse", "HEAD"], { cwd: SHELL_ROOT, encoding: "utf8" }).trim();
}

export function ensureCleanSource() {
  const repoRoot = path.resolve(SHELL_ROOT, "..");
  const tracked = execFileSync("git", ["status", "--porcelain", "--untracked-files=no"], {
    cwd: SHELL_ROOT,
    encoding: "utf8",
  }).trim();
  const untracked = execFileSync(
    "git",
    [
      "ls-files",
      "--others",
      "--exclude-standard",
      "--",
      "desktop/scripts",
      "desktop/build",
      "manager-gpui",
      "runtime/src",
    ],
    { cwd: repoRoot, encoding: "utf8" },
  ).trim();
  if (tracked || untracked)
    throw new Error("release builds require a clean tracked and source worktree");
}

// KOS-233: the Engine is built from the same commit/version as Desktop
// (build-backend.mjs). This catches a stale `.tmp/engine.next` left over
// from an earlier, differently-versioned build — the exact class of bug that
// let a published Engine 0.1.3 silently ship under a local 0.9.38 Desktop.
export function verifyEngineArtifact(version, commit) {
  const file = path.join(SHELL_ROOT, ".tmp", "engine.next", "engine-manifest.json");
  if (!existsSync(file)) throw new Error(`Engine manifest is missing: ${file}`);
  const manifest = JSON.parse(readFileSync(file, "utf8"));
  if (manifest.version !== version)
    throw new Error(
      `Engine manifest version ${manifest.version} does not match release version ${version} — rerun build:backend`,
    );
  if (manifest.source_commit !== commit)
    throw new Error(
      `Engine manifest source_commit does not match HEAD — rerun build:backend (stale .tmp/engine.next?)`,
    );
  return manifest;
}

function compareSemver(a, b) {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i += 1) if (pa[i] !== pb[i]) return pa[i] - pb[i];
  return 0;
}

// Reject a release that isn't strictly newer than the latest published tag —
// this is what makes "one product, one version" hold across the whole
// history, not just at build time. Network-dependent, so it is the one check
// `--local`/MUNDUS_RELEASE_LOCAL skip: an offline or throwaway local build has
// no way to reach the GitHub API and does not need this guarantee.
export async function assertVersionIsPublishable({ platform, version, fetchImpl = fetch }) {
  const repository = platform === "win" ? "makekosmos/desktop" : "makekosmos/desktop-mac";
  const response = await fetchImpl(`https://api.github.com/repos/${repository}/releases/latest`);
  if (!response.ok)
    throw new Error(
      `could not read the latest published ${repository} release (HTTP ${response.status}) — pass --local for an offline/local build`,
    );
  const data = await response.json();
  const latest = String(data.tag_name ?? "").replace(/^v/, "");
  if (!/^\d+\.\d+\.\d+$/.test(latest))
    throw new Error(`unexpected latest ${repository} tag: ${data.tag_name}`);
  if (compareSemver(version, latest) <= 0)
    throw new Error(
      `release version ${version} must be greater than the latest published ${latest}`,
    );
}

// Release builds are cut from `main` HEAD only — this is what "no local flag"
// buys: the published binary can always be reproduced from a plain `git
// checkout main`. `--local`/MUNDUS_RELEASE_LOCAL is for testing a build off a
// feature branch (e.g. this very branch before it merges).
export function assertBuildingFromMain(
  repoRoot,
  currentBranch = () =>
    execFileSync("git", ["rev-parse", "--abbrev-ref", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
    }).trim(),
) {
  const branch = currentBranch();
  if (branch !== "main")
    throw new Error(
      `release builds must run from main HEAD (current branch: ${branch}) — pass --local (or set MUNDUS_RELEASE_LOCAL=1) for an offline/local build off another ref`,
    );
}

export async function runReleasePreflight({ platform, local = false }) {
  if (platform !== "win") throw new Error(`Unknown platform "${platform}"`);
  const version = getVersion(platform);
  ensureCleanSource();
  const commit = currentCommit();
  if (!local) {
    assertBuildingFromMain(path.resolve(SHELL_ROOT, ".."));
    await assertVersionIsPublishable({ platform, version });
  }
  const bom = await deriveReleaseBom(path.resolve(SHELL_ROOT, ".."), platform, commit);
  verifyEngineArtifact(version, commit);
  return { platform, version, currentCommit: commit, bom };
}

export function resolvePreflightArgs(args, env = process.env) {
  const index = args.indexOf("--platform");
  return {
    platform: index === -1 ? undefined : args[index + 1],
    local: args.includes("--local") || env.MUNDUS_RELEASE_LOCAL === "1",
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { platform, local } = resolvePreflightArgs(process.argv.slice(2));
  runReleasePreflight({ platform, local })
    .then(({ platform: checkedPlatform, version, bom }) =>
      console.log(`[release-preflight] PASS ${checkedPlatform} v${version} BOM ${bom.value.id}`),
    )
    .catch((error) => {
      console.error(
        `[release-preflight] FATAL: ${error instanceof Error ? error.message : String(error)}`,
      );
      process.exitCode = 1;
    });
}
