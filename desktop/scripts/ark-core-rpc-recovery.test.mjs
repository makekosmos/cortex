import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import {
  ARK_CORE_REVISION,
  ensureArkCoreRpc,
  installRoot,
  publishToTarget,
} from "./ark-core-rpc.mjs";

async function withTempRoot(name, callback) {
  const root = path.join(tmpdir(), `kosmos-ark-core-rpc-${name}-${process.pid}-${Date.now()}`);
  mkdirSync(root, { recursive: true });
  try {
    return await callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

const binaryName = `ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`;

test("target recovers a backup left after process death", () =>
  withTempRoot("target-recovery", (root) => {
    const source = path.join(root, "source"),
      targetDir = path.join(root, "target"),
      target = path.join(targetDir, binaryName),
      backup = `${target}.crash.old-`;
    mkdirSync(targetDir, { recursive: true });
    writeFileSync(backup, "old");
    writeFileSync(source, "new");
    assert.equal(readFileSync(publishToTarget(source, targetDir), "utf8"), "new");
    assert.equal(existsSync(backup), false);
    assert.equal(
      readdirSync(targetDir).some((name) => name.includes(".tmp-")),
      false,
    );
  }));

test("cache recovers a complete backup left after process death", () =>
  withTempRoot("cache-recovery", (root) => {
    const cacheRoot = path.join(root, "cache"),
      keyRoot = installRoot(true, [], cacheRoot),
      backup = `${keyRoot}.crash.old-`;
    mkdirSync(path.join(backup, "bin"), { recursive: true });
    writeFileSync(path.join(backup, "bin", binaryName), "old");
    writeFileSync(path.join(backup, ".complete"), `${ARK_CORE_REVISION}\n`);
    const result = ensureArkCoreRpc({ debug: true, cacheRoot, cargoCommand: "missing-cargo" });
    assert.equal(readFileSync(result, "utf8"), "old");
    assert.equal(existsSync(backup), false);
  }));

test("cache replaces an incomplete canonical with a complete backup", () =>
  withTempRoot("cache-reconcile", (root) => {
    const cacheRoot = path.join(root, "cache"),
      keyRoot = installRoot(true, [], cacheRoot),
      backup = `${keyRoot}.crash.old-`;
    mkdirSync(path.join(keyRoot, "bin"), { recursive: true });
    writeFileSync(path.join(keyRoot, "bin", binaryName), "corrupt");
    writeFileSync(path.join(keyRoot, ".complete"), "wrong\n");
    mkdirSync(path.join(backup, "bin"), { recursive: true });
    writeFileSync(path.join(backup, "bin", binaryName), "old");
    writeFileSync(path.join(backup, ".complete"), `${ARK_CORE_REVISION}\n`);
    const result = ensureArkCoreRpc({ debug: true, cacheRoot, cargoCommand: "missing-cargo" });
    assert.equal(readFileSync(result, "utf8"), "old");
    assert.equal(readFileSync(path.join(keyRoot, ".complete"), "utf8").trim(), ARK_CORE_REVISION);
  }));
