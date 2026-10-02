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
    await rm(file);
    assert.throws(() => readReleaseVersion({ root }), /cannot read/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("writeReleaseVersion writes only the win entry and validates first", async () => {
  const root = await tempRepo();
  try {
    assert.throws(() => writeReleaseVersion("0.10.x", { root }), /MAJOR\.MINOR\.PATCH/);
    writeReleaseVersion("0.10.2", { root });
    const file = path.join(root, "desktop", "release-versions.json");
    assert.deepEqual(JSON.parse(await readFile(file, "utf8")), { win: "0.10.2" });
    assert.equal(readReleaseVersion({ root }), "0.10.2");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

// release-version.mjs is the single owner of desktop/release-versions.json:
// no other script may read or write it. Test files are exempt — they build
// fixture copies under temp roots and read the real file as a contract input.
test("only release-version.mjs reads or writes release-versions.json", async () => {
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
  assert.deepEqual(offenders, []);
});
