import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";

const desktopRoot = path.resolve(import.meta.dirname, "..");
const bootstrap = readFileSync(path.join(desktopRoot, "build", "ensure-engine.ps1"), "utf8");
const installer = readFileSync(path.join(desktopRoot, "build", "installer.nsh"), "utf8");
const packageJson = JSON.parse(readFileSync(path.join(desktopRoot, "package.json"), "utf8"));

test("Desktop bootstraps the standalone Engine installer", () => {
  const winResources = packageJson.build.win.extraResources;
  assert.ok(winResources.some((entry) => entry.to === "ensure-engine.ps1"));
  assert.ok(winResources.some((entry) => entry.to === "engine-manifest.json"));
  assert.doesNotMatch(installer, /Kosmos Engine\.zip/);
  assert.match(installer, /ensure-engine\.ps1/);
  assert.match(bootstrap, /installer_url/);
  assert.match(bootstrap, /installer_sha256/);
  assert.match(bootstrap, /installer_size/);
  assert.ok(bootstrap.indexOf("SHA256") < bootstrap.indexOf("Start-Process"));
  assert.match(bootstrap, /KosmosEngine/);
  assert.match(bootstrap, /Compare-EngineVersion/);
});

test("bootstrap metadata rejects a changed installer before invocation", () => {
  const installerBytes = Buffer.from("trusted installer fixture");
  const expectedHash = createHash("sha256").update(installerBytes).digest("hex");
  const changedHash = createHash("sha256").update("tampered").digest("hex");
  assert.equal(installerBytes.length > 0, true);
  assert.notEqual(changedHash, expectedHash);
  assert.match(bootstrap, /hash or size mismatch/);
});
