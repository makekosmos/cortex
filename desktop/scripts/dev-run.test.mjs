import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  processInfo,
  processIdentityFromLock,
  sameProcessIdentity,
  sameProcessStartTime,
  stopProcessTree,
} from "./dev-run-process.mjs";
import { acquirePortLease, releasePortLease } from "./dev-run-port.mjs";
import { redactText } from "./redaction.mjs";
import { createRunManifest, readRunManifest, resetRun, writeRunManifest } from "./dev-run.mjs";

test("run manifests are unique and written atomically", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-run-test-"));
  try {
    const first = createRunManifest("desktop", root);
    const second = createRunManifest("desktop", root);
    assert.notEqual(first.runId, second.runId);
    assert.notEqual(first.dataDir, second.dataDir);
    assert.notEqual(first.userDataDir, second.userDataDir);
    assert.notEqual(first.ports.shell, second.ports.shell);
    const file = path.join(first.runRoot, "manifest.json");
    writeRunManifest(file, first);
    assert.deepEqual(readRunManifest(file, root), first);
    assert.equal(readFileSync(file, "utf8").endsWith("\n"), true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("reset rejects paths outside the recognized test root", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-run-test-"));
  const outside = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-outside-"));
  try {
    writeFileSync(path.join(outside, "keep.txt"), "keep", "utf8");
    const manifest = createRunManifest("desktop", root);
    manifest.dataDir = outside;
    const file = path.join(manifest.runRoot, "manifest.json");
    mkdirSync(manifest.runRoot, { recursive: true });
    writeFileSync(file, JSON.stringify(manifest));
    assert.throws(() => readRunManifest(file, root), /canonical/);
    assert.throws(() => resetRun(file, root), /canonical|outside test root/);
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(outside, { recursive: true, force: true });
  }
});

test("reset rejects a run-root junction escape", (t) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-run-test-"));
  const outside = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-outside-"));
  try {
    writeFileSync(path.join(outside, "keep.txt"), "keep", "utf8");
    const manifest = createRunManifest("desktop", root);
    const file = path.join(manifest.runRoot, "manifest.json");
    mkdirSync(manifest.runRoot, { recursive: true });
    writeRunManifest(file, manifest);
    writeFileSync(path.join(outside, "manifest.json"), JSON.stringify(manifest));
    rmSync(manifest.runRoot, { recursive: true, force: true });
    try {
      symlinkSync(outside, manifest.runRoot, "junction");
    } catch {
      t.skip("junction creation is unavailable");
      return;
    }
    assert.throws(() => resetRun(file, root), /outside test root/);
    assert.equal(readFileSync(path.join(outside, "keep.txt"), "utf8"), "keep", "outside untouched");
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(outside, { recursive: true, force: true });
  }
});

test("port leases are unique across concurrent runs", async () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-port-test-"));
  const leases = await Promise.all(Array.from({ length: 8 }, () => acquirePortLease(root)));
  try {
    assert.equal(new Set(leases.map((lease) => lease.port)).size, leases.length);
  } finally {
    leases.forEach((lease) => releasePortLease(lease.file, root));
    rmSync(root, { recursive: true, force: true });
  }
});

test(
  "process cleanup rejects forged identity and kills the owned process",
  { timeout: 15_000 },
  async () => {
    const child = spawn(
      process.execPath,
      ["-e", "setTimeout(() => {}, 30000); // --kosmos-run-id=process-test"],
      {
        windowsHide: true,
      },
    );
    try {
      await new Promise((resolve) => child.once("spawn", resolve));
      const info = processInfo(child.pid);
      assert.ok(info);
      assert.deepEqual(stopProcessTree(child.pid, "forged", info.commandLine), []);
      assert.ok(processInfo(child.pid));
      assert.deepEqual(
        stopProcessTree(child.pid, info.startTime, `${info.commandLine} forged`),
        [],
      );
      assert.ok(processInfo(child.pid));
      assert.notDeepEqual(stopProcessTree(child.pid, info.startTime, info.commandLine), []);
    } finally {
      if (child.exitCode === null && child.signalCode === null) {
        await new Promise((resolve) => {
          child.once("exit", resolve);
          child.kill();
        });
      }
    }
  },
);

test("diagnostic redaction is bounded and removes secrets, emails, paths, and hashes", () => {
  const value = redactText(
    `token=secret@example.com password=hunter2 C:\\Users\\alice\\notes 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef ${"x".repeat(20000)}`,
  );
  assert.ok(value.includes("[REDACTED]"));
  assert.ok(!value.includes("secret@example.com"));
  assert.ok(!value.includes("C:\\Users\\alice"));
  assert.ok(value.length <= 16 * 1024);
});

test("owned cleanup kills markerless descendants", { timeout: 15_000 }, async () => {
  const runRoot = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-run-test-"));
  const code = `const {spawn}=require("node:child_process"); spawn(process.execPath,["-e","process.on('SIGTERM',()=>setTimeout(()=>process.exit(0),250)); setTimeout(()=>{},30000)"]); process.on("SIGTERM",()=>setTimeout(()=>process.exit(0),250)); setTimeout(()=>{},30000)`;
  const child = spawn(process.execPath, ["-e", code], { windowsHide: true });
  try {
    await new Promise((resolve) => child.once("spawn", resolve));
    const info = processInfo(child.pid);
    assert.ok(info);
    const stopped = stopProcessTree(child.pid, info.startTime, info.commandLine);
    assert.ok(stopped.length >= 2);
    assert.equal(processInfo(child.pid), null);
    rmSync(runRoot, { recursive: true, force: true });
  } finally {
    if (child.exitCode === null && child.signalCode === null) {
      if (process.platform === "win32") {
        try {
          execFileSync("taskkill.exe", ["/PID", String(child.pid), "/T", "/F"], {
            stdio: "ignore",
          });
        } catch {}
      } else child.kill();
    }
    rmSync(runRoot, { recursive: true, force: true });
  }
});

test("backend lock identity rejects a reused PID", { timeout: 15_000 }, async () => {
  const info = processInfo(process.pid);
  assert.ok(info);
  assert.equal(sameProcessStartTime(info.startTime, info.startTime), true);
  assert.equal(sameProcessStartTime(info.startTime, "1970-01-01T00:00:00.000Z"), false);
  const child = spawn(process.execPath, ["-e", "setTimeout(() => {}, 30000)"], {
    windowsHide: true,
  });
  try {
    await new Promise((resolve) => child.once("spawn", resolve));
    const childInfo = processInfo(child.pid);
    assert.ok(childInfo);
    assert.equal(sameProcessStartTime(info.startTime, childInfo.startTime), false);
  } finally {
    if (child.exitCode === null && child.signalCode === null) child.kill();
  }
});

test(
  "RFC backend locks accept owned processes and reject a newer reused PID",
  { timeout: 15_000 },
  async () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-lock-test-"));
    const lockPath = path.join(root, "engine.lock.json");
    const executable = process.execPath;
    const launchStartedAt = Date.now() - 1_000;
    const child = spawn(
      process.execPath,
      ["-e", "setTimeout(() => {}, 30000); // kepler-backend", "--", "--start"],
      {
        windowsHide: true,
      },
    );
    try {
      await new Promise((resolve) => child.once("spawn", resolve));
      const info = processInfo(child.pid);
      assert.ok(info);
      assert.ok(Number.isFinite(info.createdAtMs));
      writeFileSync(
        lockPath,
        JSON.stringify({ pid: child.pid, started_at: new Date(Date.now()).toISOString() }),
      );
      const owned = processIdentityFromLock(lockPath, launchStartedAt, executable);
      assert.ok(owned);
      assert.equal(sameProcessIdentity(processInfo(child.pid), owned), true);
      assert.notDeepEqual(
        stopProcessTree(owned.pid, owned.startTime, owned.commandLine, "backend"),
        [],
      );
      assert.equal(processInfo(child.pid), null);

      const reused = spawn(
        process.execPath,
        ["-e", "setTimeout(() => {}, 30000)", "--", "--start"],
        { windowsHide: true },
      );
      try {
        await new Promise((resolve) => reused.once("spawn", resolve));
        const reusedInfo = processInfo(reused.pid);
        assert.ok(reusedInfo);
        const lockTime = new Date(reusedInfo.createdAtMs - 1).toISOString();
        writeFileSync(lockPath, JSON.stringify({ pid: reused.pid, started_at: lockTime }));
        assert.equal(
          processIdentityFromLock(lockPath, reusedInfo.createdAtMs - 1_000, executable),
          null,
        );
      } finally {
        if (reused.exitCode === null && reused.signalCode === null) reused.kill();
      }
    } finally {
      if (child.exitCode === null && child.signalCode === null) child.kill();
      rmSync(root, { recursive: true, force: true });
    }
  },
);

test("port allocation retries when a candidate is occupied", async () => {
  const server = createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const occupied = server.address().port;
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-port-test-"));
  try {
    const lease = await acquirePortLease(root, [occupied, occupied + 1]);
    assert.equal(lease.port, occupied + 1);
    releasePortLease(lease.file, root);
  } finally {
    server.close();
    rmSync(root, { recursive: true, force: true });
  }
});

test("port lease release rejects a leases junction escape", (t) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-port-test-"));
  const outside = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-outside-"));
  try {
    writeFileSync(path.join(outside, "23456.lease"), "owned", "utf8");
    try {
      symlinkSync(outside, path.join(root, "leases"), "junction");
    } catch {
      t.skip("junction creation is unavailable");
      return;
    }
    assert.throws(
      () => releasePortLease(path.join(root, "leases", "23456.lease"), root),
      /outside test root/,
    );
    assert.equal(readFileSync(path.join(outside, "23456.lease"), "utf8"), "owned");
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(outside, { recursive: true, force: true });
  }
});
