import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { bytes, documentHash } from "./release-utils.mjs";
import { deriveReleaseBom, RELEASE_BOM_FILE } from "./release-bom.mjs";

const COMMIT = "a".repeat(40);
const repoRoot = path.resolve(import.meta.dirname, "..", "..");

async function fixture({
  packageManager = "pnpm@12.4.1",
  win = "0.10.0",
  node = "24.15.0",
  rust = "1.95.0",
  api = "1.0.0",
} = {}) {
  const root = await mkdtemp(path.join(os.tmpdir(), "mundus-bom-"));
  await mkdir(path.join(root, "desktop"));
  await mkdir(path.join(root, "runtime", "crates", "engine-base", "src"), { recursive: true });
  await writeFile(path.join(root, "package.json"), JSON.stringify({ packageManager }));
  await writeFile(path.join(root, "desktop", "release-versions.json"), JSON.stringify({ win }));
  await writeFile(path.join(root, "toolchain.json"), JSON.stringify({ node, rust }));
  await writeFile(
    path.join(root, "runtime", "crates", "engine-base", "src", "protocol_version.rs"),
    `pub const API_VERSION: &str = "${api}";\n`,
  );
  return root;
}

test("the BOM is derived entirely from the checkout", async () => {
  const bom = await deriveReleaseBom(await fixture(), "win", COMMIT);
  assert.deepEqual(bom.value, {
    schema_version: 2,
    id: "mundus-desktop-0.10.0-win",
    release: { version: "0.10.0", channel: "production", platform: "win" },
    source: {
      repository: "makekosmos/cortex",
      commit: COMMIT,
      toolchain: {
        pnpm: "12.4.1",
        node: "24.15.0",
        rust: "1.95.0",
        target: "x86_64-pc-windows-msvc",
      },
    },
    compatibility: { engine_api: "1.0.0" },
  });
  assert.deepEqual(bom.bytes, bytes(bom.value));
  assert.equal(bom.digest, documentHash(bom.bytes));
  assert.equal(RELEASE_BOM_FILE, "release-bom.v2.json");
});

test("the same checkout always yields the same BOM bytes", async () => {
  const root = await fixture();
  const first = await deriveReleaseBom(root, "win", COMMIT);
  const second = await deriveReleaseBom(root, "win", COMMIT);
  assert.equal(first.digest, second.digest);
  const other = await deriveReleaseBom(root, "win", "b".repeat(40));
  assert.notEqual(first.digest, other.digest);
});

test("the real repository pins derive a valid BOM", async () => {
  const bom = await deriveReleaseBom(repoRoot, "win", COMMIT);
  const versions = JSON.parse(
    await readFile(path.join(repoRoot, "desktop", "release-versions.json"), "utf8"),
  );
  assert.equal(bom.value.release.version, versions.win);
  assert.equal(bom.value.id, `mundus-desktop-${versions.win}-win`);
});

test("an unknown platform or a malformed commit is rejected", async () => {
  const root = await fixture();
  await assert.rejects(() => deriveReleaseBom(root, "mac", COMMIT), /Unknown platform "mac"/);
  await assert.rejects(() => deriveReleaseBom(root, "win", "abc"), /40-character/);
  await assert.rejects(() => deriveReleaseBom(root, "win", "A".repeat(40)), /40-character/);
});

test("every pin must be present and semantic", async () => {
  const cases = [
    [{ packageManager: "npm@10.0.0" }, /packageManager pnpm pin/],
    [{ win: null }, /release-versions\.json win/],
    [{ node: "24" }, /toolchain\.json node/],
    [{ rust: null }, /toolchain\.json rust/],
    [{ api: "one" }, /runtime API_VERSION/],
  ];
  for (const [overrides, error] of cases) {
    const root = await fixture(overrides);
    await assert.rejects(() => deriveReleaseBom(root, "win", COMMIT), error);
  }
});
