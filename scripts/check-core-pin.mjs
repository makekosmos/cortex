#!/usr/bin/env node
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";

const retiredRevision = "6692038ed5c0cb7052f2b5ffc3e206358bb0a10c";
const retiredPath = "core/ark/crates/ark-core/rust";
const subtreePath = "core/crates/ark-core";
const files = [
  "Cargo.toml",
  "runtime/Cargo.toml",
  "desktop/scripts/ark-core-source.mjs",
  "Cargo.lock",
];

const contents = new Map();
if (existsSync("core/.git"))
  throw new Error("core/ must belong to Cortex, without a nested Git repository");
for (const file of files) contents.set(file, (await readFile(file, "utf8")).replace(/\r\n/g, "\n"));
for (const [file, content] of contents) {
  if (content.includes(retiredRevision))
    throw new Error(`${file} still pins retired Core revision`);
  if (content.includes(retiredPath)) throw new Error(`${file} still references retired Core path`);
}

if (!existsSync(`${subtreePath}/Cargo.toml`))
  throw new Error(`in-tree Core subtree is missing: ${subtreePath}`);
const subtreeManifest = (await readFile(`${subtreePath}/Cargo.toml`, "utf8")).replace(
  /\r\n/g,
  "\n",
);
if (!/^name = "ark-core"$/m.test(subtreeManifest))
  throw new Error(`${subtreePath}/Cargo.toml is not the ark-core crate`);

const workspace = contents.get("Cargo.toml");
if (!workspace.includes(`"${subtreePath}"`))
  throw new Error(`Cargo.toml workspace members do not include ${subtreePath}`);

const runtime = contents.get("runtime/Cargo.toml");
if (runtime.includes("github.com/makekosmos/core"))
  throw new Error("runtime/Cargo.toml still pins ark-core from the remote Core repository");
if (!runtime.includes('path = "../core/crates/ark-core", package = "ark-core"'))
  throw new Error("runtime/Cargo.toml does not depend on the in-tree ark-core subtree");

const source = contents.get("desktop/scripts/ark-core-source.mjs");
if (!source.includes(`"${subtreePath}"`))
  throw new Error("ark-core source module does not point at the in-tree subtree");

const lock = contents.get("Cargo.lock");
const arkPackage = lock.match(/\[\[package\]\]\nname = "ark-core"\n[^[]*/)?.[0];
if (!arkPackage) throw new Error("Cargo.lock does not contain an ark-core package");
if ((lock.match(/name = "ark-core"/g) ?? []).length !== 1)
  throw new Error("Cargo.lock has an unexpected ark-core package count");
if (/source = /.test(arkPackage))
  throw new Error("Cargo.lock resolves ark-core from a source other than the workspace path");
console.log(
  "Core ownership passed: ark-core is local Cortex source, without a nested Git repository.",
);
