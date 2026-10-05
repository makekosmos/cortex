import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { verifyLocalReleaseChannel, verifyLocalReleaseManifest } from "./release-channel-local.mjs";
import { legacyChannelYml, manifestBytes } from "./release-manifest.mjs";
import { resolveVerifyTarget } from "./verify-release-target.mjs";

const hash = (bytes) => createHash("sha512").update(bytes).digest("base64");

test("verifies every local channel artifact before publication", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "mundus-channel-"));
  try {
    const installer = Buffer.from("installer");
    await writeFile(path.join(directory, "Mundus-Setup-1.2.3.exe"), installer);
    await writeFile(
      path.join(directory, "latest.yml"),
      `version: 1.2.3\nfiles:\n  - url: Mundus-Setup-1.2.3.exe\n    sha512: ${hash(installer)}\n    size: ${installer.length}\npath: Mundus-Setup-1.2.3.exe\nsha512: ${hash(installer)}\n`,
    );
    assert.doesNotThrow(() => verifyLocalReleaseChannel(directory, "1.2.3"));
    await writeFile(path.join(directory, "Mundus-Setup-1.2.3.exe"), "tampered");
    assert.throws(() => verifyLocalReleaseChannel(directory, "1.2.3"), /artifact mismatch/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("the mac channel file is latest-mac.yml and the default stays latest.yml", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "mundus-channel-mac-"));
  try {
    const archive = Buffer.from("mac-archive");
    await writeFile(path.join(directory, "Mundus-1.2.3.dmg"), archive);
    const body = `version: 1.2.3\nfiles:\n  - url: Mundus-1.2.3.dmg\n    sha512: ${hash(archive)}\n    size: ${archive.length}\npath: Mundus-1.2.3.dmg\nsha512: ${hash(archive)}\n`;
    await writeFile(path.join(directory, "latest-mac.yml"), body);
    assert.doesNotThrow(() => verifyLocalReleaseChannel(directory, "1.2.3", "mac"));
    assert.throws(
      () => verifyLocalReleaseChannel(directory, "1.2.3"),
      /ENOENT|no such file|latest\.yml/i,
    );
    assert.deepEqual(resolveVerifyTarget(["node", "verify-release-channel.mjs"]), {
      platform: "win",
      repo: "makekosmos/cortex",
      channelFile: "latest.yml",
      manifestFile: "manifest.json",
      version: null,
    });
    assert.deepEqual(
      resolveVerifyTarget(["node", "verify-release-channel.mjs", "--platform", "mac"]),
      {
        platform: "mac",
        repo: "makekosmos/cortex",
        channelFile: "latest-mac.yml",
        manifestFile: "manifest.json",
        version: null,
      },
    );
    assert.throws(
      () => resolveVerifyTarget(["node", "verify-release-channel.mjs", "--platform", "linux"]),
      /Unknown platform/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

function localManifest(installer, { platform = "win", file = "Mundus-Setup-1.2.3.exe" } = {}) {
  return {
    schema: "mundus-release-manifest",
    schema_version: 1,
    product: "mundus",
    version: "1.2.3",
    channel: "production",
    source: {
      repository: "makekosmos/cortex",
      commit: "a".repeat(40),
      toolchain: { pnpm: "12.4.1", node: "24.15.0", rust: "1.95.0" },
    },
    compatibility: { engine_api: "1.0.0" },
    platforms: {
      [platform]: {
        file,
        url: `https://github.com/makekosmos/cortex/releases/download/v1.2.3/${file}`,
        size: installer.length,
        sha512: hash(installer),
        target: "x86_64-pc-windows-msvc",
        commit: "a".repeat(40),
        released_at: "2026-10-05T10:00:00.000Z",
      },
    },
  };
}

test("manifest.json is checked against the installer and the dual-publish yml (KOS-350)", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "mundus-manifest-local-"));
  try {
    const installer = Buffer.from("installer");
    const manifest = localManifest(installer);
    await writeFile(path.join(directory, "Mundus-Setup-1.2.3.exe"), installer);
    await writeFile(path.join(directory, "manifest.json"), manifestBytes(manifest));
    // Cutover mode needs only manifest.json.
    assert.equal(
      verifyLocalReleaseManifest(directory, "1.2.3", "win", { dual: false }).version,
      "1.2.3",
    );
    // Dual mode also requires a latest.yml that agrees with the manifest.
    assert.throws(
      () => verifyLocalReleaseManifest(directory, "1.2.3", "win", { dual: true }),
      /ENOENT|latest\.yml/,
    );
    await writeFile(path.join(directory, "latest.yml"), legacyChannelYml(manifest, "win"));
    assert.doesNotThrow(() =>
      verifyLocalReleaseManifest(directory, "1.2.3", "win", { dual: true }),
    );
    assert.throws(() => verifyLocalReleaseManifest(directory, "1.2.4", "win"), /version/);
    assert.throws(() => verifyLocalReleaseManifest(directory, "1.2.3", "mac"), /no mac entry/);
    await writeFile(path.join(directory, "Mundus-Setup-1.2.3.exe"), "tampered");
    assert.throws(
      () => verifyLocalReleaseManifest(directory, "1.2.3", "win", { dual: false }),
      /artifact mismatch/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("a legacy yml that drifted from manifest.json fails the dual-publish check", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "mundus-manifest-drift-"));
  try {
    const installer = Buffer.from("installer");
    const other = Buffer.from("other-build");
    await writeFile(path.join(directory, "Mundus-Setup-1.2.3.exe"), installer);
    await writeFile(path.join(directory, "Other-1.2.3.exe"), other);
    const manifest = localManifest(installer);
    await writeFile(path.join(directory, "manifest.json"), manifestBytes(manifest));
    await writeFile(
      path.join(directory, "latest.yml"),
      legacyChannelYml(localManifest(other, { file: "Other-1.2.3.exe" }), "win"),
    );
    assert.throws(
      () => verifyLocalReleaseManifest(directory, "1.2.3", "win", { dual: true }),
      /drifted from manifest\.json/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
