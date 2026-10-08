import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";

const scriptPath = path.join(import.meta.dirname, "build-macos-native.mjs");
const repoRoot = path.resolve(import.meta.dirname, "..", "..");

test("build-macos-native exits before swiftc off macOS", () => {
  const result = spawnSync(process.execPath, [scriptPath], { encoding: "utf8" });
  if (process.platform === "darwin") {
    // A Mac host is expected to compile. This assertion only pins the no-op.
    return;
  }
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "");
});

test("every Swift helper the build script compiles is in the tree", async () => {
  const source = await readFile(scriptPath, "utf8");
  assert.match(source, /process\.platform !== "darwin"/);
  const sources = [...source.matchAll(/sources: \["([^"]+)"\]/g)].map((match) => match[1]);
  assert.deepEqual(sources, [
    "hotkey-hold-monitor.swift",
    "capture-hotkey.swift",
    "audio-capturer.swift",
    "microphone-access.swift",
    "speech-recognizer.swift",
    "input-monitoring-request.swift",
    "get-selected-text.swift",
    "paste-text.swift",
  ]);
  for (const name of sources)
    assert.equal(existsSync(path.join(repoRoot, "runtime", "native", "macos", name)), true, name);
});
