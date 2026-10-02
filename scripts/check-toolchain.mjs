#!/usr/bin/env node
// toolchain.json is the single source for the Rust version: CI installs it and
// the release BOM records it. rust-toolchain.toml only mirrors it so rustup
// picks the same toolchain locally — without the mirror the local gate can
// pass on a newer rustc while CI fails (KOS-317). `--write` regenerates the
// mirror after a toolchain.json bump, keeping one number to edit.
import { readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

const TOOLCHAIN_JSON = "toolchain.json";
const RUST_TOOLCHAIN_TOML = "rust-toolchain.toml";

export function renderRustToolchain(channel) {
  return `# Channel is the local mirror of toolchain.json "rust" — CI installs that pin,
# so rustup must select the same version or local gates drift from CI.
# scripts/check-toolchain.mjs enforces the match; bump toolchain.json and run
# \`node scripts/check-toolchain.mjs --write\` to regenerate this file.
[toolchain]
channel = "${channel}"
profile = "minimal"
components = ["rustfmt", "clippy"]
`;
}

export function pinnedChannel(toml) {
  const match = toml.match(/^channel\s*=\s*"([^"]+)"\s*$/m);
  if (!match) throw new Error(`${RUST_TOOLCHAIN_TOML} does not pin a channel`);
  return match[1];
}

export function expectedChannel(toolchainJson) {
  const rust = String(JSON.parse(toolchainJson).rust ?? "");
  if (!/^\d+\.\d+\.\d+$/.test(rust)) throw new Error(`${TOOLCHAIN_JSON} has no semver "rust" pin`);
  return rust;
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
if (isMain) {
  const expected = expectedChannel(await readFile(TOOLCHAIN_JSON, "utf8"));
  if (process.argv[2] === "--write") {
    await writeFile(RUST_TOOLCHAIN_TOML, renderRustToolchain(expected));
    console.log(`${RUST_TOOLCHAIN_TOML} regenerated from ${TOOLCHAIN_JSON}: ${expected}`);
  } else {
    const actual = pinnedChannel(await readFile(RUST_TOOLCHAIN_TOML, "utf8"));
    if (actual !== expected)
      throw new Error(
        `${RUST_TOOLCHAIN_TOML} pins ${actual} but ${TOOLCHAIN_JSON} pins ${expected}; ` +
          `run \`node scripts/check-toolchain.mjs --write\``,
      );
    console.log(`Toolchain pins agree: ${TOOLCHAIN_JSON} and ${RUST_TOOLCHAIN_TOML} = ${expected}`);
  }
}
