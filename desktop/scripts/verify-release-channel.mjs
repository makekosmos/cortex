#!/usr/bin/env node
// Verifies that a published GitHub release's channel file (latest.yml) is
// consistent with the actual uploaded installer assets.
//
// Background: electron-updater verifies the sha512 field in the channel file
// against the downloaded installer before applying the update. If the release
// was published in multiple non-atomic passes (e.g. the installer was
// re-uploaded without regenerating the channel file), the checksums desync and
// every client gets "sha512 checksum mismatch" — auto-update is dead for that
// release.
//
// This happened on v0.5.3: the installer was re-uploaded from build B while
// latest.yml still described build A. This script detects that exact scenario.
//
// Usage:
//   node scripts/verify-release-channel.mjs                  # win, version from the pinned release version
//   node scripts/verify-release-channel.mjs 0.5.3            # win, explicit version (positional, backward-compat)
//   node scripts/verify-release-channel.mjs --platform win
//   node scripts/verify-release-channel.mjs --platform mac   # cortex / latest-mac.yml
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
// Repo and channel file come from release-repos.mjs (win → makekosmos/cortex
// / latest.yml, mac → makekosmos/cortex / latest-mac.yml). --repo
// overrides the repo for bridge runs and does not change the channel file.
// Omitting --platform stays on win, which is what publish-release.mjs calls.
//
// Exit codes:
//   0  — all checks PASS
//   1  — one or more checks FAIL
//
// Retry behaviour (for post-publish use):
//   The script retries the initial channel file fetch up to MAX_RETRIES times
//   with RETRY_DELAY_MS between attempts, so a few seconds of GH release asset
//   propagation lag does not cause a false-fail right after `electron-builder
//   --publish always` completes.

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { env } from "./brand.mjs";
import { readReleaseVersion } from "./release-version.mjs";
import { resolveVerifyTarget } from "./verify-release-target.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SHELL_ROOT = path.resolve(__dirname, "..");

// ─── Configuration ────────────────────────────────────────────────────────────

// Retry config: channel file may lag a few seconds after electron-builder publish.
const MAX_RETRIES = 5;
const RETRY_DELAY_MS = 4_000;

// Timestamp desync threshold. If the spread between asset upload times exceeds
// this, it strongly suggests artifacts came from different builds.
// Choice: we WARN (not hard-fail) on spread > WARN_THRESHOLD_MS alone, but
// HARD-FAIL on spread > HARD_FAIL_THRESHOLD_MS. Any hash/size mismatch
// combined with ANY spread above WARN_THRESHOLD_MS also hard-fails.
// Rationale: a spread of a few seconds is normal (GitHub processes assets
// sequentially), but multiple minutes means a separate upload session.
const WARN_THRESHOLD_MS = 3 * 60 * 1_000; // 3 minutes
const HARD_FAIL_THRESHOLD_MS = 10 * 60 * 1_000; // 10 minutes

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

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Resolve gh CLI path. */
function resolveGh() {
  if (env("GH_PATH") && existsSync(env("GH_PATH"))) {
    return env("GH_PATH");
  }
  const candidates = [
    "C:/Program Files/GitHub CLI/gh.exe",
    "C:/Program Files (x86)/GitHub CLI/gh.exe",
  ];
  for (const c of candidates) {
    if (existsSync(c)) return c;
  }
  return "gh"; // assume PATH
}

const GH = resolveGh();

/** Check whether gh CLI is available and authenticated (read-only probe). */
function ghAvailable() {
  try {
    const r = spawnSync(GH, ["auth", "status"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    return r.status === 0;
  } catch {
    return false;
  }
}

/** Fetch asset timestamps from GitHub API for the given release tag.
 *  Returns a Map<assetName, {created_at, updated_at}> or null on failure. */
function fetchAssetTimestamps(ownerRepo, tag) {
  const r = spawnSync(
    GH,
    [
      "api",
      `repos/${ownerRepo}/releases/tags/${tag}`,
      "-q",
      "[.assets[] | {name: .name, created_at: .created_at, updated_at: .updated_at}]",
    ],
    { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
  if (r.status !== 0) {
    warn(`gh api failed for ${tag}:`, r.stderr.slice(0, 200));
    return null;
  }
  try {
    const arr = JSON.parse(r.stdout);
    const map = new Map();
    for (const a of arr) {
      map.set(a.name, { created_at: a.created_at, updated_at: a.updated_at });
    }
    return map;
  } catch {
    warn("failed to parse gh api response");
    return null;
  }
}

// ─── latest.yml parser ────────────────────────────────────────────────────────
// Dependency-free hand-parser for the flat structure electron-builder emits.
// We deliberately do NOT reach for js-yaml: whether it is hoisted into
// node_modules varies by machine / package manager, so relying on it would
// make this guard behave differently across devices and checkouts. The format
// below is fixed and simple, so a targeted parser is deterministic everywhere.

/**
 * Hand-parser for the flat latest.yml format electron-builder emits.
 *
 * Handles:
 *   version: 0.5.3
 *   files:
 *     - url: Mundus-Setup-0.5.3.exe
 *       sha512: CX4w...==
 *       size: 122041993
 *   path: Mundus-Setup-0.5.3.exe
 *   sha512: CX4w...==
 *   releaseDate: '2026-06-18T12:18:15.656Z'
 *
 * Returns { version, files: [{url, sha512, size}] }
 */
function parseLatestYmlByHand(text) {
  const lines = text.split(/\r?\n/);
  const result = { version: null, files: [] };
  let inFiles = false;
  let currentFile = null;

  for (let raw of lines) {
    // Strip inline comments
    const line = raw.replace(/#.*$/, "");
    const trimmed = line.trim();
    if (!trimmed) continue;

    // Top-level key detection (no leading spaces)
    if (/^\S/.test(line)) {
      // Commit any buffered file entry before switching section
      if (currentFile) {
        result.files.push(currentFile);
        currentFile = null;
      }

      const m = /^(\w+)\s*:\s*(.*)$/.exec(trimmed);
      if (!m) {
        inFiles = false;
        continue;
      }
      const key = m[1];
      const val = m[2].replace(/^['"]|['"]$/g, "").trim();

      if (key === "version") {
        result.version = val;
        inFiles = false;
      } else if (key === "files") {
        inFiles = true;
      } else {
        inFiles = false;
      }
      continue;
    }

    if (!inFiles) continue;

    // List item start: "  - url: ..."
    const itemStart = /^\s+-\s+(\w+)\s*:\s*(.*)$/.exec(line);
    if (itemStart) {
      if (currentFile) result.files.push(currentFile);
      currentFile = {};
      const key = itemStart[1];
      const val = itemStart[2].replace(/^['"]|['"]$/g, "").trim();
      assignFileField(currentFile, key, val);
      continue;
    }

    // Continuation property under current list item: "    sha512: ..."
    const contProp = /^\s+(\w+)\s*:\s*(.*)$/.exec(line);
    if (contProp && currentFile) {
      const key = contProp[1];
      const val = contProp[2].replace(/^['"]|['"]$/g, "").trim();
      assignFileField(currentFile, key, val);
    }
  }

  if (currentFile) result.files.push(currentFile);
  return result;
}

function assignFileField(obj, key, val) {
  if (key === "size") {
    obj.size = parseInt(val, 10);
  } else {
    obj[key] = val;
  }
}

function parseLatestYml(text) {
  return parseLatestYmlByHand(text);
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

/**
 * Fetches a URL, retrying up to `maxRetries` times with `delayMs` between
 * attempts. Returns the response text on success.
 */
async function fetchTextWithRetry(url, maxRetries, delayMs) {
  let lastErr;
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      const res = await fetch(url);
      if (!res.ok) {
        throw new Error(`HTTP ${res.status} ${res.statusText}`);
      }
      return await res.text();
    } catch (err) {
      lastErr = err;
      if (attempt < maxRetries) {
        log(
          `attempt ${attempt}/${maxRetries} failed (${err.message}), retrying in ${delayMs / 1000}s...`,
        );
        await sleep(delayMs);
      }
    }
  }
  throw lastErr;
}

// ─── Main verification logic ──────────────────────────────────────────────────

async function main() {
  let request;
  try {
    request = resolveVerifyTarget(process.argv);
  } catch (error) {
    die(error instanceof Error ? error.message : String(error));
  }
  const { platform, channelFile } = request;
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
  log(`Channel file: ${channelFile}`);
  log("");

  // ── 3. Download channel file (with retry for post-publish propagation lag) ─
  const ymlUrl = `https://github.com/${ownerRepo}/releases/download/${tag}/${channelFile}`;
  log(`Fetching ${ymlUrl} (up to ${MAX_RETRIES} attempts)...`);

  let ymlText;
  try {
    ymlText = await fetchTextWithRetry(ymlUrl, MAX_RETRIES, RETRY_DELAY_MS);
  } catch (err) {
    die(
      `Failed to download ${channelFile} after ${MAX_RETRIES} attempts: ${err.message}\n\n` +
        `Fix: ensure the release ${tag} exists on ${ownerRepo} and ${channelFile} was published.` +
        (platform === "mac"
          ? `\nNote: macOS assets publish to makekosmos/cortex as latest-mac.yml (KOS-349).`
          : ``),
    );
  }

  // ── 4. Parse channel file ──────────────────────────────────────────────────
  const manifest = parseLatestYml(ymlText);
  log(`${channelFile} declares version: ${manifest.version}`);
  log(`${channelFile} file entries: ${manifest.files.length}`);
  log("");

  // ── 5. Check version field ────────────────────────────────────────────────
  let anyHardFail = false;

  if (manifest.version !== version) {
    console.error(
      `${LOG_PREFIX} FAIL  version mismatch: ${channelFile} says "${manifest.version}", expected "${version}"`,
    );
    anyHardFail = true;
  } else {
    log(`PASS  ${channelFile} version matches: ${version}`);
  }

  if (manifest.files.length === 0) {
    die(`${channelFile} has no files[] entries — cannot verify anything.`);
  }

  // ── 6. Fetch asset timestamps (degradable) ────────────────────────────────
  const ghAvail = ghAvailable();
  let assetTimestamps = null;
  if (ghAvail) {
    log(`Fetching asset timestamps via gh api for ${ownerRepo} ${tag}...`);
    assetTimestamps = fetchAssetTimestamps(ownerRepo, tag);
    if (!assetTimestamps) {
      warn("Could not fetch asset timestamps — skipping timestamp desync check.");
    }
  } else {
    warn("gh CLI not available or not authenticated — skipping timestamp desync check.");
  }

  // ── 7. Per-asset verification ─────────────────────────────────────────────
  const assetBaseUrl = `https://github.com/${ownerRepo}/releases/download/${tag}`;
  const results = [];

  for (const entry of manifest.files) {
    const { url: assetName, sha512: expectedSha, size: expectedSize } = entry;
    log(`--- Checking: ${assetName} ---`);

    if (!assetName) {
      warn("Entry has no url field — skipping.");
      continue;
    }

    const assetUrl = `${assetBaseUrl}/${assetName}`;
    let actual;
    try {
      log(`  Downloading and hashing ${assetUrl}...`);
      actual = await downloadAndHash(assetUrl);
    } catch (err) {
      console.error(`${LOG_PREFIX} FAIL  ${assetName}: download failed — ${err.message}`);
      anyHardFail = true;
      results.push({ name: assetName, pass: false, reason: `download failed: ${err.message}` });
      continue;
    }

    let entryPass = true;
    const issues = [];

    // SHA-512 check
    if (expectedSha && actual.sha512 !== expectedSha) {
      issues.push(
        `sha512 MISMATCH:\n      ${channelFile}: ${expectedSha}\n      actual:     ${actual.sha512}`,
      );
      entryPass = false;
    } else if (!expectedSha) {
      issues.push("sha512 not present in channel file entry (cannot verify)");
      // Not a hard fail — warn only
    }

    // Size check
    if (
      Object.prototype.toString.call(expectedSize) === "[object Number]" &&
      !isNaN(expectedSize)
    ) {
      if (actual.size !== expectedSize) {
        issues.push(
          `size MISMATCH:\n      ${channelFile}: ${expectedSize} bytes\n      actual:     ${actual.size} bytes`,
        );
        entryPass = false;
      }
    }

    if (!entryPass) {
      anyHardFail = true;
      for (const issue of issues) {
        console.error(`${LOG_PREFIX} FAIL  ${assetName}: ${issue}`);
      }
    } else {
      log(`  PASS  sha512 ✓  size ${actual.size} bytes ✓`);
    }

    results.push({ name: assetName, pass: entryPass, sha512: actual.sha512, size: actual.size });
  }

  // ── 8. Timestamp desync check ─────────────────────────────────────────────
  let timestampFail = false;
  if (assetTimestamps && assetTimestamps.size > 0) {
    log("");
    log("--- Timestamp desync analysis ---");

    // Collect updated_at for each asset we verified plus the channel file itself
    const times = [];
    for (const entry of manifest.files) {
      const ts = assetTimestamps.get(entry.url);
      if (ts) {
        times.push({ name: entry.url, updatedAt: new Date(ts.updated_at).getTime() });
      }
    }
    const ymlTs = assetTimestamps.get(channelFile);
    if (ymlTs) {
      times.push({ name: channelFile, updatedAt: new Date(ymlTs.updated_at).getTime() });
    }

    if (times.length >= 2) {
      const minTime = Math.min(...times.map((t) => t.updatedAt));
      const maxTime = Math.max(...times.map((t) => t.updatedAt));
      const spreadMs = maxTime - minTime;
      const spreadSec = (spreadMs / 1000).toFixed(1);

      for (const t of times) {
        const offsetSec = ((t.updatedAt - minTime) / 1000).toFixed(1);
        log(`  ${t.name}: ${new Date(t.updatedAt).toISOString()} (offset +${offsetSec}s)`);
      }

      if (spreadMs > HARD_FAIL_THRESHOLD_MS) {
        console.error(
          `${LOG_PREFIX} FAIL  asset timestamp spread is ${spreadSec}s (> ${HARD_FAIL_THRESHOLD_MS / 60000}min threshold)`,
        );
        console.error(
          `${LOG_PREFIX}       This strongly indicates artifacts were uploaded in separate sessions — they likely come from different builds.`,
        );
        timestampFail = true;
        anyHardFail = true;
      } else if (spreadMs > WARN_THRESHOLD_MS) {
        warn(
          `asset timestamp spread is ${spreadSec}s — exceeds ${WARN_THRESHOLD_MS / 60000}min warning threshold.`,
        );
        warn(
          `Artifacts may come from different build sessions. Combined with any hash/size mismatch this is a hard failure.`,
        );
        // Upgrade to hard fail if there was already a hash/size issue
        if (anyHardFail) {
          console.error(
            `${LOG_PREFIX} FAIL  timestamp desync (${spreadSec}s spread) combined with hash/size mismatch — hard failure.`,
          );
          timestampFail = true;
        }
      } else {
        log(`  PASS  asset timestamp spread: ${spreadSec}s (within acceptable range)`);
      }
    } else {
      warn("Not enough timestamp data to compute spread.");
    }
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
  if (timestampFail) {
    log("  FAIL  timestamp desync");
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
        `${LOG_PREFIX}   4. That produces an atomic set: installer + ${channelFile} from the same build.`,
      );
    } else {
      console.error(
        `${LOG_PREFIX}   3. Republish ${tag} on ${ownerRepo} as one atomic upload of ${channelFile} and its artifacts.`,
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
    log(`OVERALL: PASS — all assets consistent with ${channelFile}`);
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
