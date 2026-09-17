import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  createRunManifest,
  listRuns,
  packageManagerCommand,
  stopRun,
  writeRunManifest,
} from "./dev-run.mjs";
import { processInfo } from "../../desktop/scripts/dev-run-process.mjs";

test("packageManagerCommand honors each package's own package manager", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-pm-test-"));
  try {
    const bunPackage = path.join(root, "bunpkg");
    const pnpmPackage = path.join(root, "pnpmpkg");
    const barePackage = path.join(root, "barepkg");
    mkdirSync(bunPackage);
    mkdirSync(pnpmPackage);
    mkdirSync(barePackage);
    writeFileSync(
      path.join(bunPackage, "package.json"),
      JSON.stringify({ packageManager: "bun@1.3.14", scripts: { dev: "bun run serve" } }),
    );
    writeFileSync(
      path.join(pnpmPackage, "package.json"),
      JSON.stringify({ packageManager: "pnpm@12.4.1", scripts: { dev: "vite" } }),
    );
    writeFileSync(path.join(barePackage, "package.json"), JSON.stringify({}));
    assert.equal(packageManagerCommand(bunPackage).binary, "bun");
    assert.equal(packageManagerCommand(bunPackage).scripts.dev, "bun run serve");
    assert.equal(packageManagerCommand(pnpmPackage).binary, "pnpm");
    assert.equal(packageManagerCommand(barePackage).binary, "pnpm");
    assert.equal(packageManagerCommand(path.join(root, "missing")).binary, "pnpm");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("listRuns reports runs and their live processes", async () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-list-test-"));
  const child = spawn(process.execPath, ["-e", "setTimeout(() => {}, 30000)"], {
    windowsHide: true,
  });
  try {
    await new Promise((resolve) => child.once("spawn", resolve));
    const info = processInfo(child.pid);
    assert.ok(info);
    const manifest = createRunManifest("manager", root);
    manifest.ownedPids.push({
      pid: child.pid,
      role: "engine",
      startTime: info.startTime,
      commandLine: info.commandLine,
    });
    writeRunManifest(path.join(manifest.runRoot, "manifest.json"), manifest);
    const runs = listRuns(root);
    assert.equal(runs.length, 1);
    assert.equal(runs[0].runId, manifest.runId);
    assert.deepEqual(runs[0].running, ["engine"]);
    const stopped = stopRun(runs[0].file, root);
    assert.notDeepEqual(stopped, []);
    assert.equal(processInfo(child.pid), null);
    assert.deepEqual(listRuns(root)[0].running, []);
  } finally {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    rmSync(root, { recursive: true, force: true });
  }
});

test("listRuns tolerates missing and unreadable manifests", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-list-test-"));
  try {
    assert.deepEqual(listRuns(path.join(root, "absent")), []);
    const broken = path.join(root, "20240101000000-deadbeef");
    mkdirSync(broken, { recursive: true });
    writeFileSync(path.join(broken, "manifest.json"), "not json");
    const runs = listRuns(root);
    assert.equal(runs.length, 1);
    assert.equal(runs[0].error, "unreadable manifest");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("stopRun never kills processes the run did not spawn", async () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-dev-stop-test-"));
  const innocent = spawn(process.execPath, ["-e", "setTimeout(() => {}, 30000)"], {
    windowsHide: true,
  });
  try {
    await new Promise((resolve) => innocent.once("spawn", resolve));
    const manifest = createRunManifest("manager", root);
    manifest.ownedPids.push({
      pid: innocent.pid,
      role: "engine",
      startTime: "forged",
      commandLine: "forged",
    });
    const file = path.join(manifest.runRoot, "manifest.json");
    mkdirSync(manifest.runRoot, { recursive: true });
    writeRunManifest(file, manifest);
    assert.deepEqual(stopRun(file, root), []);
    assert.ok(processInfo(innocent.pid));
    assert.equal(existsSync(file), true);
  } finally {
    if (innocent.exitCode === null && innocent.signalCode === null) innocent.kill();
    rmSync(root, { recursive: true, force: true });
  }
});
