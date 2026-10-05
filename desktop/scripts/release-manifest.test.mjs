import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { deriveReleaseBom, RELEASE_BOM_FILE } from "./release-bom.mjs";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";
import {
  assetDownloadUrl,
  createReleaseManifest,
  DUAL_PUBLISH_LEGACY_FEEDS,
  legacyBom,
  legacyChannelProblems,
  legacyChannelYml,
  legacyFeedFiles,
  manifestBytes,
  mergeReleaseManifest,
  parseReleaseManifest,
  RELEASE_MANIFEST_FILE,
  sha512Base64,
  validateReleaseManifest,
} from "./release-manifest.mjs";

const WIN_COMMIT = "a".repeat(40);
const MAC_COMMIT = "b".repeat(40);
const AT = "2026-10-05T10:00:00.000Z";

async function fixtureRoot() {
  const root = await mkdtemp(path.join(os.tmpdir(), "mundus-manifest-"));
  await mkdir(path.join(root, "desktop"));
  await mkdir(path.join(root, "runtime", "crates", "engine-base", "src"), { recursive: true });
  await writeFile(
    path.join(root, "package.json"),
    JSON.stringify({ packageManager: "pnpm@12.4.1" }),
  );
  await writeFile(
    path.join(root, "desktop", "release-versions.json"),
    JSON.stringify({ win: "1.2.3", mac: "1.2.3" }),
  );
  await writeFile(
    path.join(root, "toolchain.json"),
    JSON.stringify({ node: "24.15.0", rust: "1.95.0" }),
  );
  await writeFile(
    path.join(root, "runtime", "crates", "engine-base", "src", "protocol_version.rs"),
    'pub const API_VERSION: &str = "1.0.0";\n',
  );
  return root;
}

function installer(name, body) {
  const data = Buffer.from(body);
  return { file: name, size: data.length, sha512: sha512Base64(data) };
}

async function manifests() {
  const root = await fixtureRoot();
  const winBom = await deriveReleaseBom(root, "win", WIN_COMMIT);
  const macBom = await deriveReleaseBom(root, "mac", MAC_COMMIT);
  const win = createReleaseManifest({
    bom: winBom,
    installer: installer("Mundus-Setup-1.2.3.exe", "exe"),
    releasedAt: AT,
  });
  const mac = createReleaseManifest({
    bom: macBom,
    installer: installer("Mundus-1.2.3.dmg", "dmg"),
    releasedAt: AT,
  });
  await rm(root, { recursive: true, force: true });
  return { win, mac, winBom, macBom };
}

test("manifest.json schema v1 carries version, source baseline and platform entries", async () => {
  const { win } = await manifests();
  assert.equal(RELEASE_MANIFEST_FILE, "manifest.json");
  assert.deepEqual(win, {
    schema: "mundus-release-manifest",
    schema_version: 1,
    product: "mundus",
    version: "1.2.3",
    channel: "production",
    source: {
      repository: "makekosmos/cortex",
      commit: WIN_COMMIT,
      toolchain: { pnpm: "12.4.1", node: "24.15.0", rust: "1.95.0" },
    },
    compatibility: { engine_api: "1.0.0" },
    platforms: {
      win: {
        file: "Mundus-Setup-1.2.3.exe",
        url: "https://github.com/makekosmos/cortex/releases/download/v1.2.3/Mundus-Setup-1.2.3.exe",
        size: 3,
        sha512: sha512Base64(Buffer.from("exe")),
        target: "x86_64-pc-windows-msvc",
        commit: WIN_COMMIT,
        released_at: AT,
      },
    },
  });
  assert.deepEqual(parseReleaseManifest(manifestBytes(win).toString("utf8")), win);
});

test("legacy BOM rendered from the manifest is byte-identical to the derived BOM", async () => {
  const { win, mac, winBom, macBom } = await manifests();
  assert.deepEqual(legacyBom(win, "win").bytes, winBom.bytes);
  assert.equal(legacyBom(win, "win").digest, winBom.digest);
  assert.equal(legacyBom(mac, "mac").digest, macBom.digest);
});

test("legacy latest.yml rendered from the manifest passes the channel checker", async () => {
  const { win } = await manifests();
  const dir = await mkdtemp(path.join(os.tmpdir(), "mundus-manifest-yml-"));
  try {
    await writeFile(path.join(dir, "Mundus-Setup-1.2.3.exe"), "exe");
    await writeFile(path.join(dir, "latest.yml"), legacyChannelYml(win, "win"));
    const channel = verifyLocalReleaseChannel(dir, "1.2.3", "win");
    assert.deepEqual(legacyChannelProblems(win, "win", channel), []);
    assert.match(legacyChannelYml(win, "win"), new RegExp(`releaseDate: '${AT}'`));
    const drifted = { ...channel, files: [{ ...channel.files[0], size: 99 }] };
    assert.deepEqual(legacyChannelProblems(win, "win", drifted), ["size differs from manifest"]);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("dual-publish emits yml (+ bom from the creator) and the cutover emits none", () => {
  assert.equal(DUAL_PUBLISH_LEGACY_FEEDS, true);
  assert.deepEqual(legacyFeedFiles("win"), ["latest.yml", RELEASE_BOM_FILE]);
  assert.deepEqual(legacyFeedFiles("mac"), ["latest-mac.yml"]);
  assert.deepEqual(legacyFeedFiles("win", { dual: false }), []);
  assert.deepEqual(legacyFeedFiles("mac", { dual: false }), []);
});

test("windows creates the release manifest; mac merges into it without rewriting win fields", async () => {
  const { win, mac } = await manifests();
  assert.deepEqual(mergeReleaseManifest(null, win, "win"), win);
  assert.throws(() => mergeReleaseManifest(null, mac, "mac"), /no manifest\.json yet/);
  const merged = mergeReleaseManifest(win, mac, "mac");
  assert.deepEqual(Object.keys(merged.platforms).sort(), ["mac", "win"]);
  assert.equal(merged.source.commit, WIN_COMMIT);
  assert.deepEqual(merged.platforms.win, win.platforms.win);
  assert.equal(merged.platforms.mac.commit, MAC_COMMIT);
  assert.equal(merged.platforms.mac.target, "aarch64-apple-darwin");
  // A Windows retry after mac attached keeps the mac entry.
  const retried = mergeReleaseManifest(merged, win, "win");
  assert.deepEqual(retried.platforms.mac, merged.platforms.mac);
  // The legacy win BOM is still the win build's even after the merge.
  assert.equal(legacyBom(merged, "win").digest, legacyBom(win, "win").digest);
});

test("merge fails closed on another version or a different release commit", async () => {
  const { win, mac } = await manifests();
  const otherCommit = structuredClone(win);
  otherCommit.source.commit = "c".repeat(40);
  otherCommit.platforms.win.commit = "c".repeat(40);
  assert.throws(() => mergeReleaseManifest(otherCommit, win, "win"), /built from/);
  const otherVersion = structuredClone(win);
  otherVersion.version = "1.2.4";
  otherVersion.platforms.win.url = assetDownloadUrl(
    "makekosmos/cortex",
    "1.2.4",
    "Mundus-Setup-1.2.3.exe",
  );
  assert.throws(() => mergeReleaseManifest(otherVersion, mac, "mac"), /does not match/);
});

test("validation rejects unknown schema versions and unsafe entries", async () => {
  const { win } = await manifests();
  const cases = [
    [(m) => (m.schema_version = 2), /schema_version/],
    [(m) => (m.schema = "other"), /schema/],
    [(m) => (m.source.commit = "abc"), /source\.commit/],
    [(m) => (m.platforms = {}), /at least one/],
    [(m) => (m.platforms.win.file = "../evil.exe"), /safe asset name/],
    [(m) => (m.platforms.win.url = "https://evil.example/x.exe"), /url/],
    [(m) => (m.platforms.win.sha512 = "abc"), /sha512/],
    [(m) => (m.platforms.win.size = 0), /size/],
    [(m) => (m.platforms.beos = m.platforms.win), /unknown platform/],
  ];
  for (const [mutate, error] of cases) {
    const copy = structuredClone(win);
    mutate(copy);
    assert.throws(() => validateReleaseManifest(copy), error);
  }
  assert.throws(() => parseReleaseManifest("{not json"), /not valid JSON/);
  // Additive unknown keys are allowed (readers ignore them).
  assert.doesNotThrow(() => validateReleaseManifest({ ...structuredClone(win), notes: "x" }));
});

test("emitReleaseFeeds: dual writes manifest + legacy feeds that a receipt/publish accepts", async () => {
  const { emitReleaseFeeds } = await import("./release-feeds.mjs");
  const { createReceipt, assertExactArtifactSet, verifyReceiptArtifacts } =
    await import("./release-receipt.mjs");
  const { readFile } = await import("node:fs/promises");
  const root = await fixtureRoot();
  const out = await mkdtemp(path.join(os.tmpdir(), "mundus-feeds-"));
  try {
    const bom = await deriveReleaseBom(root, "win", WIN_COMMIT);
    const installerFile = path.join(out, "Mundus-Setup-1.2.3.exe");
    await writeFile(installerFile, "real installer bytes");
    const { manifest, files } = await emitReleaseFeeds({
      outputDir: out,
      platform: "win",
      version: "1.2.3",
      bom,
      installerFile,
    });
    assert.deepEqual(
      files.map((file) => path.basename(file)),
      ["Mundus-Setup-1.2.3.exe", "manifest.json", "latest.yml", RELEASE_BOM_FILE],
    );
    // No drift: legacy BOM bytes == derived BOM, yml == manifest entry.
    assert.deepEqual(await readFile(path.join(out, RELEASE_BOM_FILE)), bom.bytes);
    assert.equal(
      await readFile(path.join(out, "latest.yml"), "utf8"),
      legacyChannelYml(manifest, "win"),
    );
    const receipt = await createReceipt({
      outputDir: out,
      platform: "win",
      version: "1.2.3",
      currentCommit: WIN_COMMIT,
      bom,
      files,
    });
    assert.doesNotThrow(() =>
      assertExactArtifactSet(receipt, [
        "Mundus-Setup-1.2.3.exe",
        RELEASE_MANIFEST_FILE,
        ...legacyFeedFiles("win"),
      ]),
    );
    assert.equal((await verifyReceiptArtifacts(receipt, out)).length, 4);
  } finally {
    await rm(root, { recursive: true, force: true });
    await rm(out, { recursive: true, force: true });
  }
});

test("emitReleaseFeeds: cutover (dual off) writes only manifest.json; local skips the bom", async () => {
  const { emitReleaseFeeds } = await import("./release-feeds.mjs");
  const root = await fixtureRoot();
  const out = await mkdtemp(path.join(os.tmpdir(), "mundus-feeds-cut-"));
  try {
    const installerFile = path.join(out, "Mundus-1.2.3.dmg");
    await writeFile(installerFile, "dmg bytes");
    const macBom = await deriveReleaseBom(root, "mac", MAC_COMMIT);
    const cut = await emitReleaseFeeds({
      outputDir: out,
      platform: "mac",
      version: "1.2.3",
      bom: macBom,
      installerFile,
      dual: false,
    });
    assert.deepEqual(
      cut.files.map((file) => path.basename(file)),
      ["Mundus-1.2.3.dmg", "manifest.json"],
    );
    const winInstaller = path.join(out, "Mundus-Setup-1.2.3.exe");
    await writeFile(winInstaller, "exe bytes");
    const local = await emitReleaseFeeds({
      outputDir: out,
      platform: "win",
      version: "1.2.3",
      bom: await deriveReleaseBom(root, "win", WIN_COMMIT),
      installerFile: winInstaller,
      local: true,
    });
    assert.deepEqual(
      local.files.map((file) => path.basename(file)),
      ["Mundus-Setup-1.2.3.exe", "manifest.json", "latest.yml"],
    );
  } finally {
    await rm(root, { recursive: true, force: true });
    await rm(out, { recursive: true, force: true });
  }
});
