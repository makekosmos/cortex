import assert from "node:assert/strict";
import { mkdir, mkdtemp, readdir, readFile, writeFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { readReleaseVersion, writeReleaseVersion } from "./release-version.mjs";

async function tempRepo() {
  const root = await mkdtemp(path.join(os.tmpdir(), "mundus-release-version-"));
  await mkdir(path.join(root, "desktop"), { recursive: true });
  return root;
}

test("readReleaseVersion returns the validated win version", async () => {
  const root = await tempRepo();
  try {
    const file = path.join(root, "desktop", "release-versions.json");
    await writeFile(file, JSON.stringify({ win: "0.10.1" }) + "\n");
    assert.equal(readReleaseVersion({ root }), "0.10.1");
    await writeFile(file, JSON.stringify({ win: "0.10.x" }) + "\n");
    assert.throws(() => readReleaseVersion({ root }), /release-versions\.json win/);
    // Surrounding whitespace is rejected, never trimmed into a release tag.
    for (const padded of [" 0.10.1", "0.10.1 ", "0.10.1\n", "\t0.10.1"]) {
      await writeFile(file, JSON.stringify({ win: padded }) + "\n");
      assert.throws(() => readReleaseVersion({ root }), /release-versions\.json win/);
    }
    await rm(file);
    assert.throws(() => readReleaseVersion({ root }), /cannot read/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

// KOS-322: the installer smoke stamps a version newer than the latest
// published release without touching the pin file. The override is a single
// env var the release pipeline never sets; an invalid value is rejected with
// the same validation as the pin.
test("MUNDUS_SMOKE_VERSION overrides the pin and is validated", async () => {
  const root = await tempRepo();
  try {
    const file = path.join(root, "desktop", "release-versions.json");
    await writeFile(file, JSON.stringify({ win: "0.10.3" }) + "\n");
    process.env.MUNDUS_SMOKE_VERSION = "0.10.4";
    try {
      assert.equal(readReleaseVersion({ root }), "0.10.4");
      process.env.MUNDUS_SMOKE_VERSION = "0.10.x";
      assert.throws(() => readReleaseVersion({ root }), /MUNDUS_SMOKE_VERSION/);
    } finally {
      delete process.env.MUNDUS_SMOKE_VERSION;
    }
    assert.equal(readReleaseVersion({ root }), "0.10.3");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("writeReleaseVersion writes only the win entry and validates first", async () => {
  const root = await tempRepo();
  try {
    assert.throws(() => writeReleaseVersion("0.10.x", { root }), /MAJOR\.MINOR\.PATCH/);
    for (const padded of [" 0.10.2", "0.10.2 ", "0.10.2\n", "\t0.10.2"])
      assert.throws(() => writeReleaseVersion(padded, { root }), /MAJOR\.MINOR\.PATCH/);
    writeReleaseVersion("0.10.2", { root });
    const file = path.join(root, "desktop", "release-versions.json");
    assert.deepEqual(JSON.parse(await readFile(file, "utf8")), { win: "0.10.2" });
    assert.equal(readReleaseVersion({ root }), "0.10.2");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

// KOS-307: the two pins are independent. A Windows write must not invent,
// drop, or rewrite mac — that was the parity-bump failure mode, and it
// would block a Windows-only nightly on the mac channel.
test("writeReleaseVersion preserves the other channel", async () => {
  const root = await tempRepo();
  try {
    const file = path.join(root, "desktop", "release-versions.json");
    await writeFile(file, JSON.stringify({ win: "0.10.3", mac: "0.5.1" }, null, 2) + "\n");
    writeReleaseVersion("0.10.4", { root, platform: "win" });
    assert.deepEqual(JSON.parse(await readFile(file, "utf8")), { win: "0.10.4", mac: "0.5.1" });
    writeReleaseVersion("0.5.2", { root, platform: "mac" });
    assert.deepEqual(JSON.parse(await readFile(file, "utf8")), { win: "0.10.4", mac: "0.5.2" });
    assert.equal(readReleaseVersion({ root, platform: "win" }), "0.10.4");
    assert.equal(readReleaseVersion({ root, platform: "mac" }), "0.5.2");
    await writeFile(file, JSON.stringify({ win: "0.10.4", mac: "0.5.x" }) + "\n");
    assert.throws(() => writeReleaseVersion("0.10.5", { root }), /release-versions\.json mac/);
    await writeFile(file, JSON.stringify({ win: "0.10.4", linux: "1.0.0" }) + "\n");
    assert.throws(() => writeReleaseVersion("0.10.5", { root }), /unknown key/);
    assert.throws(() => readReleaseVersion({ root, platform: "linux" }), /Unknown platform/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("the mac pin is the same product version as win", () => {
  const root = path.resolve(import.meta.dirname, "..", "..");
  const win = readReleaseVersion({ root, platform: "win" });
  // 0.5.1 was the last makekosmos/desktop-mac tag, not a second product.
  assert.equal(readReleaseVersion({ root, platform: "mac" }), win);
  assert.equal(win, "0.10.3");
});

// release-version.mjs is the single owner of desktop/release-versions.json:
// no other script may read or write it. Test files are exempt — they build
// fixture copies under temp roots and read the real file as a contract input.
// On the Rust side the only reader is runtime/crates/pe-version-info
// (product_version, consumed by runtime/build.rs and manager-gpui/build.rs);
// runtime/crates/engine-base/src/build_info.rs is exempted explicitly — it only doc-comments the
// file name while describing where MUNDUS_PRODUCT_VERSION comes from.
test("only release-version.mjs and pe-version-info touch release-versions.json", async () => {
  const repoRoot = path.resolve(import.meta.dirname, "..", "..");
  const offenders = [];
  for (const dir of ["desktop/scripts", "scripts"]) {
    for (const entry of await readdir(path.join(repoRoot, dir), { recursive: true })) {
      if (!/\.(?:mjs|ts)$/.test(entry) || entry.endsWith(".test.mjs")) continue;
      const file = path.join(dir, entry);
      const source = await readFile(path.join(repoRoot, file), "utf8");
      if (
        source.includes("release-versions.json") &&
        file !== path.join("desktop", "scripts", "release-version.mjs")
      )
        offenders.push(file);
    }
  }
  for (const dir of ["runtime", "manager-gpui", "core"]) {
    for (const entry of await readdir(path.join(repoRoot, dir), { recursive: true })) {
      // Vendored crates and build output are not ours to police.
      const segments = entry.split(/[\\/]/);
      if (segments.includes("vendor") || segments.includes("target")) continue;
      if (!entry.endsWith(".rs")) continue;
      const file = path.join(dir, entry);
      if (
        file.startsWith(path.join("runtime", "crates", "pe-version-info")) ||
        file === path.join("runtime", "crates", "engine-base", "src", "build_info.rs")
      )
        continue;
      const source = await readFile(path.join(repoRoot, file), "utf8");
      if (source.includes("release-versions.json")) offenders.push(file);
    }
  }
  assert.deepEqual(offenders, []);
});
