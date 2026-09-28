#!/usr/bin/env node
// Brand-rename regression gate (KOS-266): `git grep -i -E "kosmos|kepler"`
// must only surface
//   1. lines marked `MIGRATION(KOS-267)` — Kosmos→Mundus migration code,
//   2. matches covered by a persisted-identifier allowlist pattern
//      (scripts/brand-allowlist.json, documented in
//      docs/brand-legacy-identifiers.md),
//   3. files explicitly allowlisted as historical records / migration tests,
//   4. changelog / release-notes history.
//
// Anything else is an unauthorized brand leak and fails the check.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { checkEnv } from "./check-plan-commands.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const allowlist = JSON.parse(
  readFileSync(path.join(root, "scripts", "brand-allowlist.json"), "utf8"),
);

const MIGRATION_MARKER = /MIGRATION\(KOS-267\)/;
const HIT = /kosmos|kepler/i;
const patterns = allowlist.patterns.map((entry) => ({
  regex: new RegExp(entry.pattern, "gi"),
  reason: entry.reason,
}));
const allowedFiles = new Set(allowlist.files.map((entry) => entry.path));
// Changelog / release-note history is allowed to mention old names.
const HISTORY_FILE = /(^|\/)(CHANGELOG|HISTORY|RELEASE[_-]NOTES)[^/]*\.(?:md|mdx|txt)$/i;
// Vendored or generated trees are not Cortex-owned brand surface.
const VENDOR_FILE = /^(?:manager-gpui\/vendor\/|node_modules\/)/;

function git(args) {
  return execFileSync("git", args, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    env: checkEnv(),
  });
}

let output;
try {
  output = git(["grep", "-inE", "kosmos|kepler", "--", "."]);
} catch (error) {
  if (error.status === 1) output = ""; // no hits at all — perfect
  else throw error;
}

const violations = [];
for (const line of output.split("\n")) {
  if (!line) continue;
  const file = line.slice(0, line.indexOf(":"));
  const body = line.slice(line.indexOf(":") + 1).replace(/^\d+:/, "");
  if (allowedFiles.has(file) || HISTORY_FILE.test(file) || VENDOR_FILE.test(file)) continue;
  if (MIGRATION_MARKER.test(body)) continue;
  let residual = body;
  for (const { regex } of patterns) residual = residual.replace(regex, "");
  if (HIT.test(residual)) violations.push(line);
}

if (violations.length) {
  console.error(
    `[check-brand] ${violations.length} unauthorized kosmos/kepler reference(s) —\n` +
      `either mark the line "MIGRATION(KOS-267): remove after 2026-11-01" or add a\n` +
      `justified pattern to scripts/brand-allowlist.json (documented in\n` +
      `docs/brand-legacy-identifiers.md):\n`,
  );
  for (const line of violations.slice(0, 60)) console.error(`  ${line}`);
  if (violations.length > 60) console.error(`  … and ${violations.length - 60} more`);
  process.exit(1);
}
console.log("[check-brand] OK — no unauthorized kosmos/kepler references");
