#!/usr/bin/env node
// Публикует extension в github.com/yoso-industries/kosmos-extensions:
//
//   1. Читает extensions/<id>/manifest.json → version.
//   2. Билдит extension через build:extensions.
//   3. Пакует dist/ + manifest.json + icon (+ README) в <id>-<version>.kext.
//   4. Computes SHA-256.
//   5. gh release create <id>-v<version> ... <kext-file> в kosmos-extensions.
//
// После publish — запусти `bun run ext:catalog -- <path>` чтобы обновить
// catalog.json (catalog генерится из gh api releases, не из локального state).
//
// Usage:
//   bun run --cwd shell ext:publish <extension-id>
//   bun run --cwd shell ext:publish --all
//
// Требуется:
//   - gh CLI: либо в PATH, либо absolute path в KEPLER_GH_PATH env.
//   - GH_TOKEN env (для gh) или предварительный `gh auth login`.

import { existsSync, mkdirSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execSync, spawnSync } from "node:child_process";
import {
  buildNativeRelease,
  fileSize,
  packageExtensionKext,
  sha256File,
} from "./extension-package-utils.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SHELL_ROOT = path.resolve(__dirname, "..");
const REPO_ROOT = path.resolve(SHELL_ROOT, "..");
const EXTENSIONS_ROOT = path.join(REPO_ROOT, "extensions");
const RELEASES_REPO = "yoso-industries/kosmos-extensions";

function die(msg) {
  console.error(`[ext:publish] ${msg}`);
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
  return "gh"; // assume PATH
}
const GH = resolveGh();

function listExtensionIds() {
  return readdirSync(EXTENSIONS_ROOT).filter((id) => {
    const m = path.join(EXTENSIONS_ROOT, id, "manifest.json");
    return existsSync(m);
  });
}

function readManifest(id) {
  const p = path.join(EXTENSIONS_ROOT, id, "manifest.json");
  if (!existsSync(p)) die(`manifest.json не найден: ${p}`);
  return JSON.parse(readFileSync(p, "utf8"));
}

function buildExtensions() {
  console.log(`[ext:publish] build:extensions...`);
  execSync(`bun run build:extensions`, { cwd: SHELL_ROOT, stdio: "inherit" });
}

function packageKext(id, manifest) {
  const outDir = path.join(SHELL_ROOT, ".tmp", "ext-publish");
  mkdirSync(outDir, { recursive: true });
  try {
    return packageExtensionKext(id, manifest, {
      extensionsRoot: EXTENSIONS_ROOT,
      repoRoot: REPO_ROOT,
      outDir,
      logPrefix: "ext:publish",
    });
  } catch (error) {
    die(error.message);
  }
}

function ghReleaseExists(tag) {
  const r = spawnSync(GH, ["release", "view", tag, "-R", RELEASES_REPO], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  return r.status === 0;
}

function ghReleaseCreate(tag, title, notes, kextPath) {
  console.log(`[ext:publish] gh release create ${tag} -R ${RELEASES_REPO}`);
  const args = [
    "release",
    "create",
    tag,
    "-R",
    RELEASES_REPO,
    "--title",
    title,
    "--notes",
    notes,
    kextPath,
  ];
  const r = spawnSync(GH, args, { stdio: "inherit" });
  if (r.status !== 0) die(`gh release create failed (exit ${r.status})`);
}

function publishOne(id, opts) {
  const manifest = readManifest(id);
  const version = manifest.version ?? "0.0.0";
  const tag = `${id}-v${version}`;
  console.log(`[ext:publish] ${id} v${version} → tag ${tag}`);

  if (ghReleaseExists(tag)) {
    if (opts.skipExisting) {
      console.log(`[ext:publish] tag ${tag} уже существует — skip`);
      return null;
    }
    die(
      `release ${tag} уже существует в ${RELEASES_REPO}. Bump version или удали release manually.`,
    );
  }

  if (!opts.skipBuild) {
    if (manifest.kind === "native") {
      buildNativeRelease(id, manifest, REPO_ROOT, "ext:publish");
    } else {
      buildExtensions();
    }
  }
  const kextPath = packageKext(id, manifest);
  const sha = sha256File(kextPath);
  const size = fileSize(kextPath);
  console.log(`[ext:publish] ${path.basename(kextPath)} ${size}B sha256=${sha.slice(0, 12)}...`);

  const title = `${manifest.name} v${version}`;
  const notes = [
    `${manifest.name} v${version}`,
    "",
    manifest.description ?? "",
    "",
    `SHA-256: \`${sha}\``,
    "",
    "Installable через Kepler Settings → Маркетплейс, либо `Kepler ext:install <url>`.",
  ].join("\n");

  ghReleaseCreate(tag, title, notes, kextPath);
  console.log(`[ext:publish] ${tag} published`);
  return { id, version, tag, kextPath, sha };
}

// --- main --------------------------------------------------------------------

const args = process.argv.slice(2);
const all = args.includes("--all");
const skipBuild = args.includes("--no-build");
const skipExisting = args.includes("--skip-existing");
const positional = args.filter((a) => !a.startsWith("--"));

if (!all && positional.length === 0) {
  die("usage: ext:publish <extension-id> | ext:publish --all [--skip-existing] [--no-build]");
}

const ids = all ? listExtensionIds() : positional;
if (ids.length === 0) die("no extensions to publish");

console.log(`[ext:publish] publish set: ${ids.join(", ")}`);

if (all && !skipBuild) {
  const manifests = ids.map((id) => [id, readManifest(id)]);
  if (manifests.some(([, manifest]) => manifest.kind !== "native")) {
    buildExtensions();
  }
  for (const [id, manifest] of manifests) {
    if (manifest.kind === "native") {
      buildNativeRelease(id, manifest, REPO_ROOT, "ext:publish");
    }
  }
}

const results = [];
for (const id of ids) {
  const r = publishOne(id, { skipBuild: all || skipBuild, skipExisting });
  if (r) results.push(r);
}

console.log(`[ext:publish] done. published: ${results.length}/${ids.length}`);
for (const r of results) {
  console.log(`  - ${r.tag} (sha256 ${r.sha.slice(0, 12)}...)`);
}
console.log(
  `[ext:publish] Не забудь: bun run --cwd shell ext:catalog -- <output-path> и push в kosmos-extensions`,
);
