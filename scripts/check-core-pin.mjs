#!/usr/bin/env node
import { readFile } from "node:fs/promises";

const expectedRevision = "169c1967a074ae6658e81d59892247b24332ce29";
const retiredRevision = "6692038ed5c0cb7052f2b5ffc3e206358bb0a10c";
const retiredPath = "core/ark/crates/ark-core/rust";
const files = [
  "runtime/Cargo.toml",
  "desktop/scripts/ark-core-rpc.mjs",
  "desktop/scripts/runtime-staging.test.mjs",
  "Cargo.lock",
];

const contents = new Map();
for (const file of files) contents.set(file, await readFile(file, "utf8"));
for (const [file, content] of contents) {
  if (content.includes(retiredRevision))
    throw new Error(`${file} still pins retired Core revision`);
  if (content.includes(retiredPath)) throw new Error(`${file} still references retired Core path`);
}

const runtime = contents.get("runtime/Cargo.toml");
if (
  !runtime.includes(
    `git = "https://github.com/makekosmos/core.git", rev = "${expectedRevision}", package = "ark-core"`,
  )
) {
  throw new Error("runtime/Cargo.toml does not pin the merged flattened Core revision");
}
const sidecar = contents.get("desktop/scripts/ark-core-rpc.mjs");
if (!sidecar.includes(`ARK_CORE_REVISION = "${expectedRevision}"`)) {
  throw new Error("sidecar installer is not aligned with the merged Core revision");
}
const lock = contents.get("Cargo.lock");
const lockedSource = `git+https://github.com/makekosmos/core.git?rev=${expectedRevision}#${expectedRevision}`;
if (!lock.includes(lockedSource))
  throw new Error("Cargo.lock does not resolve the exact Core revision");
if ((lock.match(/name = "ark-core"/g) ?? []).length !== 1)
  throw new Error("Cargo.lock has an unexpected ark-core package count");
console.log(`Core dependency pin passed: ${expectedRevision}; retired path/revision absent.`);
