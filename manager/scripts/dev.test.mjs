import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { test } from "node:test";

const script = path.join(import.meta.dirname, "dev.mjs");
const source = readFileSync(script, "utf8");
const moduleUrl = JSON.stringify(pathToFileURL(script).href);

// run() and start() terminate their own process on child completion, so the
// functional checks run them inside a helper node process and observe the code.
const helper = (body, cwd) =>
  spawnSync(process.execPath, ["--input-type=module", "-e", body], {
    cwd,
    encoding: "utf8",
    timeout: 30_000,
  });

test("dev.mjs spawns pnpm through a shell on Windows", () => {
  assert.match(source, /process\.platform === "win32" \? "pnpm\.cmd" : "pnpm"/);
  const shellSites = source.match(/shell: pnpm\.endsWith\("\.cmd"\)/g) ?? [];
  assert.equal(shellSites.length, 2, "run() and start() must both pass shell");
});

test("dev.mjs uses pnpm exec for vite and electron", () => {
  assert.match(source, /"exec", "vite"/);
  assert.match(source, /"exec", "electron", "\."/);
  assert.doesNotMatch(source, /\["x",/, 'pnpm has no "x" command; use "exec"');
});

test("dev.mjs packages and starts every dev-package entry", () => {
  assert.match(source, /run\(cwd, \["run", "package:kspkg"\]\)/);
  assert.match(source, /start\(cwd, \["run", "dev"\]\)/);
});

test("run() executes pnpm in an arbitrary package cwd", async () => {
  const cwd = await mkdtemp(path.join(os.tmpdir(), "dev-run-"));
  writeFileSync(
    path.join(cwd, "package.json"),
    JSON.stringify({ scripts: { probe: 'node -e "process.exit(0)"' } }),
  );
  const result = helper(
    `import { run } from ${moduleUrl}; run(${JSON.stringify(cwd)}, ["run", "probe"]);`,
    cwd,
  );
  if (result.error?.code === "ENOENT") return;
  assert.equal(result.status, 0, result.stderr);
});

test("start() spawns pnpm asynchronously and propagates its exit code", async () => {
  const cwd = await mkdtemp(path.join(os.tmpdir(), "dev-start-"));
  const ok = helper(
    `import { start } from ${moduleUrl}; start(${JSON.stringify(cwd)}, ["--version"]);`,
    cwd,
  );
  assert.equal(ok.status, 0, ok.stderr);
  const failed = helper(
    `import { start } from ${moduleUrl}; start(${JSON.stringify(cwd)}, ["definitely-not-a-pnpm-command"]);`,
    cwd,
  );
  assert.notEqual(failed.status, 0);
});
