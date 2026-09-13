import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmodSync, mkdirSync, readFileSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawn } from "node:child_process";
import { test } from "node:test";
import {
  ARK_CORE_REVISION,
  acquireCacheLock,
  ensureArkCoreRpc,
  installRoot,
  validatePrebuiltManifest,
} from "./ark-core-rpc.mjs";

const moduleUrl = pathToFileURL(fileURLToPath(new URL("./ark-core-rpc.mjs", import.meta.url))).href;

function tempRoot(name) {
  return path.join(tmpdir(), `kosmos-ark-core-rpc-${name}-${process.pid}-${Date.now()}`);
}

function writeFakeCargo(root) {
  const script = path.join(root, "fake-cargo.mjs");
  writeFileSync(
    script,
    `import { appendFileSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
const args = process.argv.slice(2);
const rootIndex = args.indexOf("--root");
const installRoot = args[rootIndex + 1];
appendFileSync(path.join(${JSON.stringify(root)}, "builds"), "build\\n");
Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
mkdirSync(path.join(installRoot, "bin"), { recursive: true });
writeFileSync(path.join(installRoot, "bin", ${JSON.stringify(`ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`)}), "fake-sidecar");
`,
  );
  return script;
}

function runWorker(moduleUrl, cacheRoot, fakeCargo) {
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
    let stderr = "";
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("error", reject);
    child.on("close", (status) => resolve({ status, stderr }));
  });
}

test("concurrent ensure calls build one complete cache entry and reuse it", async () => {
  const root = tempRoot("concurrency");
  mkdirSync(root, { recursive: true });
  const fakeCargo = writeFakeCargo(root);
  if (process.platform !== "win32") chmodSync(fakeCargo, 0o755);

  const results = await Promise.all([
    runWorker(moduleUrl, path.join(root, "cache"), fakeCargo),
    runWorker(moduleUrl, path.join(root, "cache"), fakeCargo),
  ]);
  assert.deepEqual(
    results.map(({ status }) => status),
    [0, 0],
  );
  assert.equal(readFileSync(path.join(root, "builds"), "utf8"), "build\n");
});

test("prebuilt manifest validates platform, arch, pinned revision, and hash", () => {
  const root = tempRoot("prebuilt");
  mkdirSync(root, { recursive: true });
  const binary = path.join(root, `ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`);
  writeFileSync(binary, "valid-sidecar");
  const sha256 = createHash("sha256").update(readFileSync(binary)).digest("hex");
  const manifest = {
    platform: process.platform,
    arch: process.arch,
    coreRevision: ARK_CORE_REVISION,
    binary: path.basename(binary),
    sha256,
  };
  const manifestPath = path.join(root, "manifest.json");
  writeFileSync(manifestPath, JSON.stringify(manifest));
  assert.equal(validatePrebuiltManifest(manifestPath), binary);

  for (const [field, value, message] of [
    ["platform", "not-this-platform", "platform"],
    ["arch", "not-this-arch", "arch"],
    ["coreRevision", "0".repeat(40), "Core revision"],
    ["sha256", "0".repeat(64), "SHA-256"],
  ]) {
    const invalid = { ...manifest, [field]: value };
    writeFileSync(manifestPath, JSON.stringify(invalid));
    assert.throws(() => validatePrebuiltManifest(manifestPath), new RegExp(message));
  }
});

test("valid prebuilt is copied, while missing prebuilt falls back to source", () => {
  const root = tempRoot("prebuilt-flow");
  mkdirSync(root, { recursive: true });
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
  const copied = ensureArkCoreRpc({
    prebuiltManifest: manifestPath,
    targetDir: path.join(root, "target"),
  });
  assert.equal(readFileSync(copied, "utf8"), "valid-sidecar");

  const fakeCargo = writeFakeCargo(root);
  if (process.platform !== "win32") chmodSync(fakeCargo, 0o755);
  const fallback = ensureArkCoreRpc({
    prebuiltManifest: path.join(root, "missing.json"),
    cacheRoot: path.join(root, "cache"),
    cargoCommand: process.execPath,
    cargoArgsPrefix: [fakeCargo],
  });
  assert.equal(readFileSync(fallback, "utf8"), "fake-sidecar");
  assert.equal(readFileSync(path.join(root, "builds"), "utf8"), "build\n");
});

test("cache keys invalidate on profile or feature changes", () => {
  const root = tempRoot("keys");
  assert.equal(installRoot(true, ["b", "a"], root), installRoot(true, ["a", "b"], root));
  assert.notEqual(installRoot(true, ["a"], root), installRoot(false, ["a"], root));
  assert.notEqual(installRoot(true, ["a"], root), installRoot(true, ["b"], root));
});

test("stale malformed lock is reclaimed", () => {
  const root = tempRoot("stale-lock");
  mkdirSync(root, { recursive: true });
  const lockPath = path.join(root, "cache.lock");
  writeFileSync(lockPath, "not-json");
  const old = new Date(Date.now() - 60 * 60 * 1000);
  utimesSync(lockPath, old, old);
  const release = acquireCacheLock(lockPath, { waitMs: 100, staleMs: 1 });
  release();
});
