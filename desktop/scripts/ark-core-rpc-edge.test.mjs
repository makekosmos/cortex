import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { test } from "node:test";
import {
  ARK_CORE_REVISION,
  acquireCacheLock,
  ensureArkCoreRpc,
  installRoot,
  publishToTarget,
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

function writeFakeCargo(root, failOnce = false) {
  const script = path.join(root, "fake-cargo.mjs");
  writeFileSync(
    script,
    `import { appendFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
const args = process.argv.slice(2), rootIndex = args.indexOf("--root"), installRoot = args[rootIndex + 1];
appendFileSync(path.join(${JSON.stringify(root)}, "builds"), "build\\n");
${
  failOnce
    ? `const marker = path.join(${JSON.stringify(root)}, "failed");
if (!existsSync(marker)) { writeFileSync(marker, "failed"); process.exit(1); }`
    : ""
}
Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100);
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

function runLockWorker(lockPath) {
  const code = `import { acquireCacheLock } from ${JSON.stringify(moduleUrl)};
const release = acquireCacheLock(process.argv[1], { waitMs: 1000, staleMs: 1 });
Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100);
release();`;
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["--input-type=module", "-e", code, lockPath], {
      stdio: "pipe",
      windowsHide: true,
    });
    child.on("error", reject);
    child.on("close", resolve);
  });
}

test("stale malformed lock is reclaimed", () =>
  withTempRoot("stale-lock", (root) => {
    const lockPath = path.join(root, "cache.lock"),
      old = new Date(Date.now() - 60 * 60 * 1000);
    writeFileSync(lockPath, "not-json");
    utimesSync(lockPath, old, old);
    acquireCacheLock(lockPath, { waitMs: 100, staleMs: 1 })();
  }));

test("concurrent stale reclaim has no global reaper lock", async () =>
  withTempRoot("stale-concurrent", async (root) => {
    const lockPath = path.join(root, "cache.lock");
    writeFileSync(lockPath, JSON.stringify({ pid: 999_999_999, token: "dead" }));
    writeFileSync(`${lockPath}.reap-leftover`, "leftover");
    assert.deepEqual(await Promise.all([runLockWorker(lockPath), runLockWorker(lockPath)]), [0, 0]);
    assert.equal(existsSync(`${lockPath}.reap-leftover`), true);
    assert.deepEqual(
      readdirSync(root).filter(
        (name) => name.startsWith("cache.lock.reap-") && name !== "cache.lock.reap-leftover",
      ),
      [],
    );
  }));

test("failed builder leaves no partial cache and can retry", () =>
  withTempRoot("builder-retry", (root) => {
    const fakeCargo = writeFakeCargo(root, true);
    if (process.platform !== "win32") chmodSync(fakeCargo, 0o755);
    const options = {
      debug: true,
      cacheRoot: path.join(root, "cache"),
      cargoCommand: process.execPath,
      cargoArgsPrefix: [fakeCargo],
    };
    assert.throws(() => ensureArkCoreRpc(options), /cargo install failed/);
    const keyRoot = installRoot(true, [], options.cacheRoot);
    assert.equal(existsSync(keyRoot), false);
    assert.equal(
      readdirSync(path.dirname(keyRoot)).some((name) => name.includes(".tmp-")),
      false,
    );
    assert.equal(readFileSync(ensureArkCoreRpc(options), "utf8"), "fake-sidecar");
  }));

test("cache publication restores an incomplete entry after rename failure", () =>
  withTempRoot("cache-rollback", (root) => {
    const cacheRoot = path.join(root, "cache"),
      keyRoot = installRoot(true, [], cacheRoot);
    mkdirSync(path.join(keyRoot, "bin"), { recursive: true });
    const cachedBinary = path.join(
      keyRoot,
      "bin",
      `ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`,
    );
    writeFileSync(cachedBinary, "old");
    const fakeCargo = writeFakeCargo(root);
    if (process.platform !== "win32") chmodSync(fakeCargo, 0o755);
    let failed = false;
    assert.throws(
      () =>
        ensureArkCoreRpc({
          debug: true,
          cacheRoot,
          cargoCommand: process.execPath,
          cargoArgsPrefix: [fakeCargo],
          renamePath: (from, to) => {
            if (!failed && to === keyRoot) {
              failed = true;
              throw new Error("cache publish failed");
            }
            renameSync(from, to);
          },
        }),
      /cache publish failed/,
    );
    assert.equal(readFileSync(cachedBinary, "utf8"), "old");
    assert.equal(
      readFileSync(
        ensureArkCoreRpc({
          debug: true,
          cacheRoot,
          cargoCommand: process.execPath,
          cargoArgsPrefix: [fakeCargo],
        }),
        "utf8",
      ),
      "fake-sidecar",
    );
  }));

test("prebuilt copy failure falls back without leaving a temporary target", () =>
  withTempRoot("prebuilt-copy", (root) => {
    const { binary, manifestPath } = writePrebuilt(root),
      targetDir = path.join(root, "target"),
      fakeCargo = writeFakeCargo(root);
    if (process.platform !== "win32") chmodSync(fakeCargo, 0o755);
    const result = ensureArkCoreRpc({
      prebuiltManifest: manifestPath,
      targetDir,
      cacheRoot: path.join(root, "cache"),
      cargoCommand: process.execPath,
      cargoArgsPrefix: [fakeCargo],
      copyFile: (source, target) => {
        writeFileSync(binary, "changed");
        copyFileSync(source, target);
      },
    });
    assert.equal(readFileSync(result, "utf8"), "fake-sidecar");
    assert.equal(
      readdirSync(targetDir).some((name) => name.includes(".tmp-")),
      false,
    );
  }));

test("target publication restores the old binary after a rename failure", () =>
  withTempRoot("publish-rollback", (root) => {
    const source = path.join(root, "source"),
      targetDir = path.join(root, "target"),
      target = path.join(targetDir, `ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`);
    writeFileSync(source, "new");
    mkdirSync(targetDir, { recursive: true });
    writeFileSync(target, "old");
    let renames = 0;
    assert.throws(
      () =>
        publishToTarget(source, targetDir, undefined, copyFileSync, (from, to) => {
          if (++renames === 2) throw new Error("publish failed");
          renameSync(from, to);
        }),
      /publish failed/,
    );
    assert.equal(readFileSync(target, "utf8"), "old");
    assert.equal(
      readdirSync(targetDir).some((name) => name.includes(".tmp-")),
      false,
    );
  }));

test("prebuilt publication failure falls back to source", () =>
  withTempRoot("prebuilt-publish", (root) => {
    const { manifestPath } = writePrebuilt(root),
      targetDir = path.join(root, "target"),
      fakeCargo = writeFakeCargo(root);
    if (process.platform !== "win32") chmodSync(fakeCargo, 0o755);
    mkdirSync(targetDir, { recursive: true });
    writeFileSync(
      path.join(targetDir, `ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`),
      "old",
    );
    let renames = 0;
    const result = ensureArkCoreRpc({
      prebuiltManifest: manifestPath,
      targetDir,
      cacheRoot: path.join(root, "cache"),
      cargoCommand: process.execPath,
      cargoArgsPrefix: [fakeCargo],
      renamePath: (from, to) => {
        if (++renames === 2) throw new Error("publish failed");
        renameSync(from, to);
      },
    });
    assert.equal(readFileSync(result, "utf8"), "fake-sidecar");
  }));

test("live and fresh malformed locks time out without deletion", () =>
  withTempRoot("lock-timeout", (root) => {
    for (const [name, contents] of [
      ["live", JSON.stringify({ pid: process.pid })],
      ["fresh", "not-json"],
    ]) {
      const lockPath = path.join(root, `${name}.lock`);
      writeFileSync(lockPath, contents);
      assert.throws(
        () => acquireCacheLock(lockPath, { waitMs: 1, staleMs: 60_000 }),
        /timed out waiting for cache lock/,
      );
      assert.equal(existsSync(lockPath), true);
    }
  }));
