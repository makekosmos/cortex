#!/usr/bin/env node
// Verifies that a published GitHub release's manifest.json (KOS-350) is
// consistent with the actual uploaded installer for one platform, that the
// stable `releases/latest/download/manifest.json` URL resolves.
//
// Background: the updater verifies the sha512 from the feed against the
// downloaded installer before applying the update. If the release was
// published in multiple non-atomic passes (e.g. the installer was re-uploaded
// without regenerating the feed), the checksums desync and every client gets
// "sha512 checksum mismatch" — auto-update is dead for that release. This
// happened on v0.5.3 (installer from build B, feed from build A).
//
// Usage:
//   node scripts/verify-release-channel.mjs                  # win, version from the pinned release version
//   node scripts/verify-release-channel.mjs 0.5.3            # win, explicit version (positional, backward-compat)
//   node scripts/verify-release-channel.mjs --platform win
//   node scripts/verify-release-channel.mjs --platform mac   # cortex / platforms.mac
//   node scripts/verify-release-channel.mjs --version 0.5.3
//   node scripts/verify-release-channel.mjs --repo owner/name   # bridge-run override
//   pnpm run verify:channel
//   pnpm run verify:channel -- 0.5.3
//
// Version resolution order:
//   1. --version flag
//   2. positional argument (backward-compat)
//   3. the pinned release version for that platform (release-version.mjs)
//   4. package.json.version (last fallback)
//
// Repo and manifest come from release-repos.mjs (win → makekosmos/cortex
// platforms.win, mac → platforms.mac). --repo overrides the repo.
// Omitting --platform stays on win, which is what publish-release.mjs calls.
//
// Exit codes:
//   0  — all checks PASS
//   1  — one or more checks FAIL
//
// Retry behaviour (for post-publish use):
//   manifest.json is polled until it carries this platform's entry and the
//   expected version, up to MANIFEST_READY_DEADLINE_MS
//   (verify-manifest-retry.mjs, cache-busted): a CDN edge can serve a
//   pre-merge copy for minutes after the other platform's upload — a stale
//   manifest used to die() instantly (KOS-377). A real mismatch still
//   fails, just after the bound.

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readReleaseVersion } from "./release-version.mjs";
import { resolveVerifyTarget } from "./verify-release-target.mjs";
import { fetchManifestUntilReady } from "./verify-manifest-retry.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SHELL_ROOT = path.resolve(__dirname, "..");

// ─── Configuration ────────────────────────────────────────────────────────────

const LOG_PREFIX = "[verify:channel]";

// ─── Helpers ──────────────────────────────────────────────────────────────────

function log(...args) {
  console.log(LOG_PREFIX, ...args);
}

function warn(...args) {
  console.warn(LOG_PREFIX, "⚠ ", ...args);
}

function die(msg) {
  console.error(LOG_PREFIX, "FATAL:", msg);
  process.exit(1);
}

// ─── Download & hash ──────────────────────────────────────────────────────────

/**
 * Downloads a URL and computes its SHA-512 (base64) and byte length by
 * streaming the response body through a crypto.createHash.
 * Avoids holding the full ~120MB in memory at once.
 */
async function downloadAndHash(url) {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} ${res.statusText} — ${url}`);
  }

  const hash = createHash("sha512");
  let byteLength = 0;

  // Node 18+ fetch returns a Web Streams ReadableStream on .body
  const reader = res.body.getReader();
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    hash.update(value);
    byteLength += value.byteLength;
  }

  return {
    sha512: hash.digest("base64"),
    size: byteLength,
  };
}

// ─── Main verification logic ──────────────────────────────────────────────────

async function main() {
  let request;
  try {
    request = resolveVerifyTarget(process.argv);
  } catch (error) {
    die(error instanceof Error ? error.message : String(error));
  }
  const { platform, manifestFile } = request;
  const ownerRepo = request.repo;

  // ── 2. Determine version & publish config ────────────────────────────────
  const pkgPath = path.join(SHELL_ROOT, "package.json");
  const pkg = JSON.parse(await readFile(pkgPath, "utf8"));

  // Version resolution order:
  //   1. --version flag
  //   2. positional arg (backward-compat)
  //   3. the pinned release version for this platform
  //   4. package.json.version (last fallback)
  let version = request.version;
  if (!version) {
    try {
      version = readReleaseVersion({ platform });
    } catch {
      // the pinned version file may not exist on very old checkouts
    }
  }
  if (!version) {
    version = pkg.version ?? null;
  }
  if (!version) {
    die(`Cannot determine version — pass --version <v> or bump the ${platform} release version`);
  }

  const tag = `v${version}`;
  log(`Platform:     ${platform}`);
  log(`Verifying release ${tag} on ${ownerRepo}`);
  log(`Manifest:     ${manifestFile}`);
  log("");

  // ── 3. Download manifest.json (with retry for post-publish propagation lag)
  // KOS-350: manifest.json is the source of truth. The other platform's
  // publish may be re-uploading a merged manifest (--clobber) right now, so
  // the retry also covers that short window.
  const assetBaseUrl = `https://github.com/${ownerRepo}/releases/download/${tag}`;
  const manifestUrl = `${assetBaseUrl}/${manifestFile}`;
  log(`Fetching ${manifestUrl} (retries until the ${platform} entry appears)...`);
  let manifest;
  try {
    manifest = (
      await fetchManifestUntilReady({
        url: manifestUrl,
        platform,
        version,
        log: (msg) => log(msg),
      })
    ).manifest;
  } catch (err) {
    die(
      `Failed to read ${manifestFile} for ${tag}: ${err.message}\n\n` +
        `Fix: ensure the release ${tag} exists on ${ownerRepo} and ${manifestFile} was published.`,
    );
  }

  // ── 4. Check version + platform entry ─────────────────────────────────────
  let anyHardFail = false;
  if (manifest.version !== version) {
    console.error(
      `${LOG_PREFIX} FAIL  version mismatch: ${manifestFile} says "${manifest.version}", expected "${version}"`,
    );
    anyHardFail = true;
  } else {
    log(`PASS  ${manifestFile} version matches: ${version}`);
  }
  log(`${manifestFile} platforms: ${Object.keys(manifest.platforms).sort().join(", ")}`);
  const entry = manifest.platforms[platform];
  if (!entry) die(`${manifestFile} has no ${platform} entry — cannot verify anything.`);
  if (manifest.source.repository !== ownerRepo)
    warn(
      `${manifestFile} source.repository is ${manifest.source.repository}, verifying ${ownerRepo}`,
    );
  log("");

  // ── 6. Installer bytes vs manifest entry ──────────────────────────────────
  const results = [];
  {
    const assetName = entry.file;
    const assetUrl = `${assetBaseUrl}/${assetName}`;
    log(`--- Checking: ${assetName} ---`);
    let actual;
    try {
      log(`  Downloading and hashing ${assetUrl}...`);
      actual = await downloadAndHash(assetUrl);
    } catch (err) {
      console.error(`${LOG_PREFIX} FAIL  ${assetName}: download failed — ${err.message}`);
      anyHardFail = true;
      results.push({ name: assetName, pass: false });
    }
    if (actual) {
      const issues = [];
      if (actual.sha512 !== entry.sha512)
        issues.push(
          `sha512 MISMATCH:\n      ${manifestFile}: ${entry.sha512}\n      actual:     ${actual.sha512}`,
        );
      if (actual.size !== entry.size)
        issues.push(
          `size MISMATCH:\n      ${manifestFile}: ${entry.size} bytes\n      actual:     ${actual.size} bytes`,
        );
      if (issues.length > 0) {
        anyHardFail = true;
        for (const issue of issues) console.error(`${LOG_PREFIX} FAIL  ${assetName}: ${issue}`);
      } else {
        log(`  PASS  sha512 ✓  size ${actual.size} bytes ✓`);
      }
      results.push({ name: assetName, pass: issues.length === 0 });
    }
  }

  // ── 7b. Stable URL: …/releases/latest/download/manifest.json ──────────────
  const stableUrl = `https://github.com/${ownerRepo}/releases/latest/download/${manifestFile}`;
  log(`--- Checking stable ${stableUrl} ---`);
  try {
    const latest = (
      await fetchManifestUntilReady({
        url: stableUrl,
        platform,
        version,
        log: (msg) => log(msg),
      })
    ).manifest;
    if (latest.version === version) log(`  PASS  stable manifest resolves to ${version}`);
    else warn(`stable manifest resolves to ${latest.version}, not ${version} (older release?)`);
    results.push({ name: `latest/${manifestFile}`, pass: true });
  } catch (err) {
    console.error(`${LOG_PREFIX} FAIL  stable ${manifestFile}: ${err.message}`);
    results.push({ name: `latest/${manifestFile}`, pass: false });
    anyHardFail = true;
  }

  // ── 9. Summary ────────────────────────────────────────────────────────────
  log("");
  log("════════════════════════════════════════");
  log(`Release: ${ownerRepo} ${tag}`);
  log("Results:");
  for (const r of results) {
    const status = r.pass ? "PASS" : "FAIL";
    log(`  ${status}  ${r.name}`);
  }

  if (anyHardFail) {
    log("════════════════════════════════════════");
    console.error(`${LOG_PREFIX} OVERALL: FAIL`);
    console.error("");
    console.error(`${LOG_PREFIX} HOW TO FIX:`);
    console.error(
      `${LOG_PREFIX}   The only safe fix is to publish a fresh version with a single atomic run.`,
    );
    console.error(`${LOG_PREFIX}   1. NEVER re-upload individual assets to an existing release.`);
    console.error(
      `${LOG_PREFIX}   2. Bump a new patch version (e.g. ${version} → ${bumpPatch(version)}).`,
    );
    if (platform === "win") {
      console.error(`${LOG_PREFIX}   3. Run one clean build+publish:  pnpm run build`);
      console.error(
        `${LOG_PREFIX}      (which ends with: node scripts/build-desktop.mjs --platform win)`,
      );
      console.error(
        `${LOG_PREFIX}   4. That produces an atomic set: installer + ${manifestFile} from the same build.`,
      );
    } else {
      console.error(
        `${LOG_PREFIX}   3. Re-run the macOS publish for ${tag}: it re-merges platforms.mac into ${manifestFile} and re-uploads its artifacts.`,
      );
      console.error(
        `${LOG_PREFIX}      The Windows installer build does not produce this channel.`,
      );
    }
    console.error(
      `${LOG_PREFIX}   Alternatively: delete ALL assets from the broken release, then re-run the build.`,
    );
    process.exit(1);
  } else {
    log("════════════════════════════════════════");
    log(`OVERALL: PASS — all assets consistent with ${manifestFile}`);
    process.exit(0);
  }
}

function bumpPatch(version) {
  const parts = String(version).split(".");
  if (parts.length !== 3) return `${version}-fixed`;
  return `${parts[0]}.${parts[1]}.${parseInt(parts[2], 10) + 1}`;
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === path.resolve(__filename);
if (isMain) {
  main().catch((e) => {
    console.error(`${LOG_PREFIX} Unhandled error:`, e?.stack ?? String(e));
    process.exit(1);
  });
}
