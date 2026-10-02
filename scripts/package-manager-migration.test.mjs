import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";

const root = path.resolve(import.meta.dirname, "..");
const packageRoots = [".", "desktop"];
const read = (file) => readFileSync(path.join(root, file), "utf8");

test("Cortex uses pnpm 12.4.1 for every package root", () => {
  for (const packageRoot of packageRoots) {
    const relative = path.join(packageRoot, "package.json");
    const packageJson = JSON.parse(read(relative));
    assert.equal(packageJson.packageManager, "pnpm@12.4.1", relative);
    assert.equal(existsSync(path.join(root, packageRoot, "bun.lock")), false, relative);
    assert.equal(existsSync(path.join(root, packageRoot, "pnpm-lock.yaml")), true, relative);
  }
});

test("Cortex command and test contracts do not require Bun", () => {
  const files = [
    "package.json",
    "desktop/package.json",
    "lefthook.yml",
    "README.md",
    "scripts/check-plan-commands.mjs",
  ];
  for (const file of files) assert.doesNotMatch(read(file), /\bbun(?:x)?\b|bun:test/, file);
  const contents = read("desktop/package.json");
  assert.doesNotMatch(contents, /\bbun(?:x)?\b|bun:test/, "desktop/package.json");
});

test("release BOM records the pnpm toolchain without changing Rust commands", () => {
  const source = read("desktop/scripts/release-bom.mjs");
  assert.match(source, /pnpm/);
  assert.doesNotMatch(source, /\bbun\b/);
  assert.match(read("package.json"), /cargo fmt --all -- --check/);
  assert.match(read("package.json"), /cargo clippy --workspace --all-targets/);
  assert.match(read("package.json"), /test:rust/);
});
