#!/usr/bin/env node
// Генерирует catalog.json для kosmos-extensions marketplace.
//
// Источник правды: GitHub releases в yoso-industries/kosmos-extensions
// (per-extension tagged: horologion-v0.3.0 ...). Per group выбирает
// highest semver. Metadata (name/description/keplerApiVersion) тянет
// из локального source — products/<id>, incubator/<id> или legacy extensions/<id>.
//
// Usage:
//   bun run --cwd platform/desktop ext:catalog -- <output-path>
//   bun run --cwd platform/desktop ext:catalog -- .tmp/kosmos-extensions/catalog.json

import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { findRepoExtensionEntry } from "./repo-extension-roots.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SHELL_ROOT = path.resolve(__dirname, "..");
const REPO_ROOT = path.resolve(SHELL_ROOT, "..", "..");
const RELEASES_REPO = "yoso-industries/kosmos-extensions";
const RAW_BASE = `https://raw.githubusercontent.com/${RELEASES_REPO}/main`;
const DOWNLOAD_BASE = `https://github.com/${RELEASES_REPO}/releases/download`;

function die(msg) {
  console.error(`[ext:catalog] ${msg}`);
  process.exit(1);
}

function resolveGh() {
  if (process.env.KEPLER_GH_PATH && existsSync(process.env.KEPLER_GH_PATH)) {
    return process.env.KEPLER_GH_PATH;
  }
  const candidates = [
    "C:/Program Files/GitHub CLI/gh.exe",
    "C:/Program Files (x86)/GitHub CLI/gh.exe",
  ];
  for (const c of candidates) if (existsSync(c)) return c;
  return "gh";
}
const GH = resolveGh();

function parseSemver(v) {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(String(v).trim());
  if (!m) return null;
  return { major: +m[1], minor: +m[2], patch: +m[3] };
}
function cmpSemver(a, b) {
  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  return a.patch - b.patch;
}

function parseTag(tag) {
  // <id>-v<x.y.z>
  const m = /^([\w][\w.-]*?)-v(\d+\.\d+\.\d+)$/.exec(tag);
  if (!m) return null;
  return { id: m[1], version: m[2] };
}

function localManifestOrNull(id) {
  const entry = findRepoExtensionEntry(REPO_ROOT, id);
  if (!entry) return null;
  try {
    return entry.manifest;
  } catch {
    return null;
  }
}

async function fetchReleases() {
  // gh release list -R ... --json — paginate manually через `gh api`.
  // Используем gh api с --paginate.
  const r = spawnSync(
    GH,
    [
      "api",
      `repos/${RELEASES_REPO}/releases`,
      "--paginate",
      "-q",
      ".[] | {tag_name, name, published_at, assets: [.assets[] | {name, browser_download_url, size, digest}]}",
    ],
    { encoding: "utf8" },
  );
  if (r.status !== 0) {
    // empty repo / no releases — graceful: return empty
    console.warn(
      `[ext:catalog] gh api releases failed (assuming empty): ${r.stderr.slice(0, 200)}`,
    );
    return [];
  }
  // jq -q вывод — последовательность JSON objects по одному в строке.
  const lines = r.stdout.split(/\r?\n/).filter((l) => l.trim().length > 0);
  const releases = [];
  for (const line of lines) {
    try {
      releases.push(JSON.parse(line));
    } catch {
      console.warn(`[ext:catalog] skip malformed line: ${line.slice(0, 80)}`);
    }
  }
  return releases;
}

async function main() {
  const outArg = process.argv[2];
  if (!outArg) die("usage: ext:catalog -- <output-path>");
  const outPath = path.resolve(outArg);

  const releases = await fetchReleases();
  console.log(`[ext:catalog] fetched ${releases.length} releases from ${RELEASES_REPO}`);

  // Group by extension id, pick highest semver.
  const latestPerId = new Map();
  for (const rel of releases) {
    const parsed = parseTag(rel.tag_name);
    if (!parsed) continue;
    const semver = parseSemver(parsed.version);
    if (!semver) continue;
    const prev = latestPerId.get(parsed.id);
    if (!prev || cmpSemver(semver, parseSemver(prev.version)) > 0) {
      latestPerId.set(parsed.id, { ...parsed, release: rel });
    }
  }

  const extensions = [];
  for (const [id, info] of latestPerId.entries()) {
    const manifest = localManifestOrNull(id);
    if (!manifest) {
      console.warn(`[ext:catalog] нет локального manifest.json для ${id} — skip`);
      continue;
    }
    const asset = info.release.assets.find((a) => a.name.endsWith(".kext"));
    if (!asset) {
      console.warn(`[ext:catalog] no .kext asset для ${info.release.tag_name} — skip`);
      continue;
    }
    // digest от gh api приходит как "sha256:..." (если GitHub его считал)
    let sha256 = null;
    if (asset.digest && typeof asset.digest === "string") {
      const m = /^sha256:([0-9a-f]{64})$/i.exec(asset.digest);
      if (m) sha256 = m[1].toLowerCase();
    }
    extensions.push({
      id,
      name: manifest.name ?? id,
      description: manifest.description ?? "",
      // author — optional. Раньше fallback'или на "yoso-industries", но это
      // вводило в заблуждение (показывался автор там где manifest его не
      // объявил). null/undefined → marketplace UI просто не рендерит автора.
      author: manifest.author ?? null,
      version: info.version,
      keplerApiVersion: manifest.keplerApiVersion ?? "^1.0.0",
      iconUrl: manifest.icon ? `${RAW_BASE}/extensions/${id}/${manifest.icon}` : null,
      downloadUrl: `${DOWNLOAD_BASE}/${info.release.tag_name}/${asset.name}`,
      sha256,
      size: asset.size ?? null,
    });
  }

  const catalog = {
    schemaVersion: 1,
    updatedAt: new Date().toISOString(),
    extensions: extensions.sort((a, b) => a.id.localeCompare(b.id)),
  };

  mkdirSync(path.dirname(outPath), { recursive: true });
  writeFileSync(outPath, JSON.stringify(catalog, null, 2) + "\n", "utf8");
  console.log(`[ext:catalog] wrote ${outPath} — ${extensions.length} extensions`);
}

main().catch((e) => die(e?.stack ?? String(e)));
