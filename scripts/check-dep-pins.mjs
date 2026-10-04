#!/usr/bin/env node
// Watches dependency pins that rot silently (KOS-346): exact `=x.y.z` version
// requirements and pre-release requirements (`-rc`, `-beta`, `-alpha`, `-pre`)
// in Cargo.toml dependency specs. cargo-deny has no lint for these, so a
// forgotten RC pin only surfaces when it breaks a release build.
//
// New pins fail this check. To land one deliberately, add it to ALLOWED below
// with a comment naming the upstream constraint that forces it — matching how
// deny.toml documents its skip list.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { pathToFileURL } from "node:url";

// "crate@<manifest dir>" → exact requirement string that is allowed.
const ALLOWED = new Map([
  // ed25519-dalek 3.0.0-rc.0: the 3.x line carries the API noq/iroh need; no
  // stable 3.x exists yet. Revisit on the first stable 3.x release.
  ["core/crates/ark-core/Cargo.toml::ed25519-dalek", "=3.0.0-rc.0"],
  // ed25519-dalek =3.0.0-rc.0 (engine dev-dep): replication test fixtures sign
  // their own documents; pinned to the same release as ark-core so tests do
  // not pull a second ed25519/curve25519 stack (moved with the KOS-334 split).
  ["runtime/Cargo.toml::ed25519-dalek", "=3.0.0-rc.0"],
  // ort 2.0.0-rc.12 (engine-dictation, local-dictation only): transcribe-rs
  // 0.3.11 builds against this rc and does not re-export the load-dynamic
  // feature, so the same rc is declared directly for feature unification.
  ["runtime/crates/engine-dictation/Cargo.toml::ort", "2.0.0-rc.12"],
  // hpke =0.14.1: the iroh/noq stack pins the same RustCrypto draft suite;
  // a caret bump splits the crypto traits.
  ["runtime/Cargo.toml::hpke", "=0.14.1"],
  // embed-resource =3.0.11: Windows resource compiler helper, pinned to keep
  // manager-gpui build output reproducible.
  ["runtime/crates/pe-version-info/Cargo.toml::embed-resource", "=3.0.11"],
  // gpui-kit/gpui-component =0.6.2: the vendored gpui fork only builds against
  // this exact upstream release.
  ["manager-gpui/Cargo.toml::gpui", "=0.6.2"],
  ["manager-gpui/Cargo.toml::gpui-component", "=0.6.2"],
]);

// Vendored/upstream trees and generated dirs are not our pins to police.
const SKIP_DIRS = new Set(["vendor", "target", "node_modules", ".tmp", ".git"]);
const PRERELEASE = /-(?:rc|beta|alpha|pre)(?:\.|\b)/;

function* cargoTomls(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (entry === "Cargo.toml") yield path;
    else if (statSync(path).isDirectory() && !SKIP_DIRS.has(entry)) yield* cargoTomls(path);
  }
}

// Finds "<dep> = \"<req>\"" and `version = "<req>"` inside `{ ... }` specs.
// [package]/[workspace.package] version lines are the crate's own version, not
// a requirement, and are skipped via section tracking.
function offendingPins(manifest) {
  const hits = [];
  let section = "";
  for (const line of readFileSync(manifest, "utf8").split("\n")) {
    const header = line.match(/^\s*\[([^\]]+)\]/);
    if (header) section = header[1].trim();
    if (section === "package" || section === "workspace.package") continue;
    const dep = line.match(/^\s*([A-Za-z0-9_-]+)\s*=\s*"([^"]+)"/);
    const inline = line.match(/^\s*([A-Za-z0-9_-]+)\s*=\s*\{[^}]*version\s*=\s*"([^"]+)"/);
    const bare = line.match(/^\s*version\s*=\s*"([^"]+)"/);
    const name = dep?.[1] ?? inline?.[1] ?? null;
    const req = dep?.[2] ?? inline?.[2] ?? bare?.[1] ?? null;
    if (!req || (!req.startsWith("=") && !PRERELEASE.test(req))) continue;
    // `version = "..."` under a [dependencies.X] table uses the table name.
    const tableDep = section.match(/(?:^|\.)dependencies\.([A-Za-z0-9_-]+)$/)?.[1];
    hits.push({ dep: name ?? tableDep ?? "(unknown)", req });
  }
  return hits;
}

export function collectViolations(root) {
  const violations = [];
  for (const manifest of cargoTomls(root)) {
    const rel = relative(root, manifest);
    for (const hit of offendingPins(manifest)) {
      const key = `${rel}::${hit.dep}`;
      if (ALLOWED.get(key) === hit.req) continue;
      violations.push(`${rel}: ${hit.dep} = "${hit.req}"`);
    }
  }
  return violations;
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
if (isMain) {
  const violations = collectViolations(process.cwd());
  if (violations.length) {
    console.error("New exact or pre-release dependency pins found:");
    for (const v of violations) console.error(`  ${v}`);
    console.error(
      "Document the upstream constraint in ALLOWED in scripts/check-dep-pins.mjs, or relax the pin.",
    );
    process.exit(1);
  }
  console.log("dep-pins ok: no undocumented exact/pre-release pins");
}
