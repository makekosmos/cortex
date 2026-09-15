import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmodSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { test } from "node:test";
import {
  ARK_CORE_REVISION,
  ensureArkCoreRpc,
  installRoot,
  validatePrebuiltManifest,
} from "./ark-core-rpc.mjs";

const moduleUrl = new URL("./ark-core-rpc.mjs", import.meta.url).href;

async function withTempRoot(name, callback) {
  const root = path.join(tmpdir(), `kosmos-ark-core-rpc-${name}-${process.pid}-${Date.now()}`);
  mkdirSync(root, { recursive: true });
  try {
    return await callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function writeFakeCargo(root) {
  const script = path.join(root, "fake-cargo.mjs");
  writeFileSync(
    script,
    `import { appendFileSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
const args = process.argv.slice(2), rootIndex = args.indexOf("--root"), installRoot = args[rootIndex + 1];
appendFileSync(path.join(${JSON.stringify(root)}, "builds"), "build\\n");
mkdirSync(path.join(installRoot, "bin"), { recursive: true });
writeFileSync(path.join(installRoot, "bin", ${JSON.stringify(`ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`)}), "fake-sidecar");
`,
  );
  return script;
}

function writePrebuilt(root) {
  const binary = path.join(root, `ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`);
  writeFileSync(binary, "valid-sidecar");
  const manifestPath = path.join(root, "manifest.json");
  writeFileSync(
    manifestPath,
    JSON.stringify({
      platform: process.platform,
      arch: process.arch,
      coreRevision: ARK_CORE_REVISION,
      binary: path.basename(binary),
      sha256: createHash("sha256").update(readFileSync(binary)).digest("hex"),
    }),
  );
  return { binary, manifestPath };
}

function runWorker(cacheRoot, fakeCargo) {
  const code = `import { ensureArkCoreRpc } from ${JSON.stringify(moduleUrl)};
ensureArkCoreRpc({ debug: true, features: ["iroh-spike"], cacheRoot: process.argv[1], cargoCommand: process.execPath, cargoArgsPrefix: [process.argv[2]] });`;
  return new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      ["--input-type=module", "-e", code, cacheRoot, fakeCargo],
      {
        stdio: "pipe",
        windowsHide: true,
      },
    );
    child.on("error", reject);
    child.on("close", resolve);
  });
}

test("concurrent ensure calls build one complete cache entry and reuse it", async () =>
  withTempRoot("concurrency", async (root) => {
    const fakeCargo = writeFakeCargo(root);
    if (process.platform !== "win32") chmodSync(fakeCargo, 0o755);
    assert.deepEqual(
      await Promise.all([
        runWorker(path.join(root, "cache"), fakeCargo),
        runWorker(path.join(root, "cache"), fakeCargo),
      ]),
      [0, 0],
    );
    assert.equal(readFileSync(path.join(root, "builds"), "utf8"), "build\n");
  }));

test("prebuilt manifest validates platform, arch, pinned revision, and hash", () =>
  withTempRoot("prebuilt", (root) => {
    const { binary, manifestPath } = writePrebuilt(root);
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    assert.equal(validatePrebuiltManifest(manifestPath), binary);
    for (const [field, value, message] of [
      ["platform", "not-this-platform", "platform"],
      ["arch", "not-this-arch", "arch"],
      ["coreRevision", "0".repeat(40), "Core revision"],
      ["sha256", "0".repeat(64), "SHA-256"],
    ]) {
      writeFileSync(manifestPath, JSON.stringify({ ...manifest, [field]: value }));
      assert.throws(() => validatePrebuiltManifest(manifestPath), new RegExp(message));
    }
  }));

test("valid prebuilt is owned, while missing prebuilt falls back to source", () =>
  withTempRoot("prebuilt-flow", (root) => {
    const { binary, manifestPath } = writePrebuilt(root);
    const copied = ensureArkCoreRpc({
      prebuiltManifest: manifestPath,
      targetDir: path.join(root, "target"),
    });
    assert.equal(readFileSync(copied, "utf8"), "valid-sidecar");
    const owned = ensureArkCoreRpc({
      prebuiltManifest: manifestPath,
      cacheRoot: path.join(root, "owned-cache"),
    });
    assert.notEqual(owned, binary);
    writeFileSync(binary, "changed-after-validation");
    assert.equal(readFileSync(owned, "utf8"), "valid-sidecar");
    const fakeCargo = writeFakeCargo(root);
    if (process.platform !== "win32") chmodSync(fakeCargo, 0o755);
    const fallback = ensureArkCoreRpc({
      prebuiltManifest: path.join(root, "missing.json"),
      cacheRoot: path.join(root, "cache"),
      cargoCommand: process.execPath,
      cargoArgsPrefix: [fakeCargo],
    });
    assert.equal(readFileSync(fallback, "utf8"), "fake-sidecar");
  }));

test("cache keys include profile, features, platform, and arch", () => {
  const root = path.join(tmpdir(), `kosmos-ark-core-rpc-keys-${process.pid}`);
  assert.equal(installRoot(true, ["b", "a"], root), installRoot(true, ["a", "b"], root));
  assert.notEqual(installRoot(true, ["a"], root), installRoot(false, ["a"], root));
  assert.notEqual(installRoot(true, ["a"], root), installRoot(true, ["b"], root));
  assert.notEqual(
    installRoot(true, ["a"], root, "linux", "x64"),
    installRoot(true, ["a"], root, "win32", "x64"),
  );
  rmSync(root, { recursive: true, force: true });
});
