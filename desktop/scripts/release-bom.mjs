// The release BOM records what one desktop release is built from. Every field
// is derived from the checkout at HEAD — nothing is hand-written — so the BOM
// can never drift from the source it describes. It ships next to the
// installer as `release-bom.v2.json`, and its digest is bound into the
// provenance and the verification receipt.
//
// Store packages are not part of it: the Engine reads the unsigned
// integrations catalog at runtime and native apps come from their own
// GitHub Releases, so no package bytes ride in the installer.

import { readFile } from "node:fs/promises";
import path from "node:path";
import { bytes, documentHash, fail, readJson, semver } from "./release-utils.mjs";
import { readReleaseVersion } from "./release-version.mjs";

export const RELEASE_BOM_SCHEMA_VERSION = 2;
export const RELEASE_BOM_FILE = `release-bom.v${RELEASE_BOM_SCHEMA_VERSION}.json`;
const REPOSITORY = "makekosmos/cortex";
const TARGETS = { win: "x86_64-pc-windows-msvc" };
const COMMIT = /^[0-9a-f]{40}$/;

export async function deriveReleaseBom(root, platform, commit) {
  const target = TARGETS[platform] ?? fail(`Unknown platform "${platform}"`);
  if (!COMMIT.test(commit)) fail("commit must be a 40-character lowercase commit");
  const packageJson = await readJson(path.join(root, "package.json"));
  const toolchain = await readJson(path.join(root, "toolchain.json"));
  const protocol = await readFile(path.join(root, "runtime", "src", "protocol_version.rs"), "utf8");

  const version = readReleaseVersion({ root });
  const pnpm = /^pnpm@(\d+\.\d+\.\d+)$/.exec(String(packageJson.packageManager ?? ""))?.[1];
  const engineApi = /pub const API_VERSION: &str = "([^"]+)"/.exec(protocol)?.[1];
  const value = {
    schema_version: RELEASE_BOM_SCHEMA_VERSION,
    id: `mundus-desktop-${version}-${platform}`,
    release: { version, channel: "production", platform },
    source: {
      repository: REPOSITORY,
      commit,
      toolchain: {
        pnpm: semver(pnpm, "package.json packageManager pnpm pin"),
        node: semver(toolchain.node, "toolchain.json node"),
        rust: semver(toolchain.rust, "toolchain.json rust"),
        target,
      },
    },
    compatibility: { engine_api: semver(engineApi, "runtime API_VERSION") },
  };
  const raw = bytes(value);
  return { value, bytes: raw, digest: documentHash(raw) };
}
