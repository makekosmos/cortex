#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { getVersion } from "./release-version.mjs";
import { loadReleaseBom } from "./release-bom.mjs";
import { documentHash } from "./package-release-utils.mjs";

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
      "desktop/src",
      "desktop/electron",
      "desktop/scripts",
      "desktop/build",
      "desktop/shared",
      "host/src",
      "host/electron",
      "manager-gpui",
      "runtime/src",
      "native-services",
    ],
    { cwd: repoRoot, encoding: "utf8" },
  ).trim();
  if (tracked || untracked)
    throw new Error("release builds require a clean tracked and source worktree");
}

export function verifyArkArtifact(bom) {
  const expected = bom.value.source.core.ark_artifact;
  const file = path.join(SHELL_ROOT, ".tmp", "runtime.next", expected.name);
  if (!existsSync(file) || statSync(file).size !== expected.size)
    throw new Error(`ARK artifact is missing or has the wrong size: ${expected.name}`);
  if (documentHash(readFileSync(file)) !== expected.sha256)
    throw new Error(`ARK artifact hash does not match BOM: ${expected.name}`);
}

export async function runReleasePreflight({ platform, bomPath }) {
  if (!["win", "mac"].includes(platform)) throw new Error(`Unknown platform "${platform}"`);
  if (!bomPath)
    throw new Error("--bom <path> or KOSMOS_RELEASE_BOM is required for release builds");
  const version = getVersion(platform);
  ensureCleanSource();
  const commit = currentCommit();
  const bom = await loadReleaseBom(bomPath, {
    root: path.resolve(SHELL_ROOT, ".."),
    platform,
    currentCommit: commit,
  });
  verifyArkArtifact(bom);
  return {
    platform,
    version,
    engineVersion:
      platform === "win"
        ? JSON.parse(
            readFileSync(path.join(SHELL_ROOT, ".tmp/engine.next/engine-manifest.json"), "utf8"),
          ).version
        : null,
    currentCommit: commit,
    bom,
  };
}

export function resolvePreflightArgs(args, env = process.env) {
  const flagValue = (flag) => {
    const index = args.indexOf(flag);
    return index === -1 ? undefined : args[index + 1];
  };
  return {
    platform: flagValue("--platform"),
    bomPath: flagValue("--bom") ?? env.KOSMOS_RELEASE_BOM,
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { platform, bomPath } = resolvePreflightArgs(process.argv.slice(2));
  runReleasePreflight({ platform, bomPath })
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
