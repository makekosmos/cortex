import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { verifyLocalReleaseManifest } from "./release-channel-local.mjs";
import { manifestBytes } from "./release-manifest.mjs";
import { resolveVerifyTarget } from "./verify-release-target.mjs";

const hash = (bytes) => createHash("sha512").update(bytes).digest("base64");

test("the verify target resolves the platform; both read manifest.json", () => {
  assert.deepEqual(resolveVerifyTarget(["node", "verify-release-channel.mjs"]), {
    platform: "win",
    repo: "makekosmos/cortex",
    manifestFile: "manifest.json",
    version: null,
  });
  assert.deepEqual(
    resolveVerifyTarget(["node", "verify-release-channel.mjs", "--platform", "mac"]),
    { platform: "mac", repo: "makekosmos/cortex", manifestFile: "manifest.json", version: null },
  );
  assert.throws(
    () => resolveVerifyTarget(["node", "verify-release-channel.mjs", "--platform", "linux"]),
    /Unknown platform/,
  );
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

test("manifest.json is checked against the installer next to it (KOS-350)", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "mundus-manifest-local-"));
  try {
    const installer = Buffer.from("installer");
    await writeFile(path.join(directory, "Mundus-Setup-1.2.3.exe"), installer);
    await writeFile(path.join(directory, "manifest.json"), manifestBytes(localManifest(installer)));
    assert.equal(verifyLocalReleaseManifest(directory, "1.2.3", "win").version, "1.2.3");
    assert.throws(() => verifyLocalReleaseManifest(directory, "1.2.4", "win"), /version/);
    assert.throws(() => verifyLocalReleaseManifest(directory, "1.2.3", "mac"), /no mac entry/);
    await writeFile(path.join(directory, "Mundus-Setup-1.2.3.exe"), "tampered");
    assert.throws(() => verifyLocalReleaseManifest(directory, "1.2.3", "win"), /artifact mismatch/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
