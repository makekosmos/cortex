import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";
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
    await writeFile(path.join(directory, "Mundus-1.2.3.zip"), archive);
    const body = `version: 1.2.3\nfiles:\n  - url: Mundus-1.2.3.zip\n    sha512: ${hash(archive)}\n    size: ${archive.length}\npath: Mundus-1.2.3.zip\nsha512: ${hash(archive)}\n`;
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
      version: null,
    });
    assert.deepEqual(
      resolveVerifyTarget(["node", "verify-release-channel.mjs", "--platform", "mac"]),
      {
        platform: "mac",
        repo: "makekosmos/desktop-mac",
        channelFile: "latest-mac.yml",
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
