import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";

const hash = (bytes) => createHash("sha512").update(bytes).digest("base64");

test("verifies every local channel artifact before publication", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "kosmos-channel-"));
  try {
    const installer = Buffer.from("installer");
    const blockmap = Buffer.from("blockmap");
    await writeFile(path.join(directory, "Kosmos-Setup-1.2.3.exe"), installer);
    await writeFile(path.join(directory, "Kosmos-Setup-1.2.3.exe.blockmap"), blockmap);
    await writeFile(
      path.join(directory, "latest.yml"),
      `version: 1.2.3\nfiles:\n  - url: Kosmos-Setup-1.2.3.exe\n    sha512: ${hash(installer)}\n    size: ${installer.length}\n  - url: Kosmos-Setup-1.2.3.exe.blockmap\n    sha512: ${hash(blockmap)}\n    size: ${blockmap.length}\npath: Kosmos-Setup-1.2.3.exe\nsha512: ${hash(installer)}\n`,
    );
    assert.doesNotThrow(() => verifyLocalReleaseChannel(directory, "win", "1.2.3"));
    await writeFile(path.join(directory, "Kosmos-Setup-1.2.3.exe.blockmap"), "tampered");
    assert.throws(() => verifyLocalReleaseChannel(directory, "win", "1.2.3"), /artifact mismatch/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
