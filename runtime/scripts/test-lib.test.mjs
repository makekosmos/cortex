import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync, existsSync, realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  aliasGateTmp,
  listBuildTreeProcesses,
  openGateTmp,
  reportGateTmpSweep,
  sweepGateTmp,
} from "./gate-tmp.mjs";

// Each test gets its own scratch dir and removes it afterwards, so the
// contract tests for "never leak into %TEMP%" do not leak into it themselves.
function scratch(t, prefix) {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
}

test("openGateTmp creates a fresh empty dir under <target>/gate-tmp", (t) => {
  const target = scratch(t, "gate-tmp-target-");
  const dir = openGateTmp(target, 1234);
  assert.equal(dir, resolve(target, "gate-tmp", "run-1234"));
  assert.ok(existsSync(dir));
  assert.deepEqual(sweepGateTmp(target, dir), []);
  assert.ok(!existsSync(dir));
});

test("sweepGateTmp removes leftover test dirs and names their contents", (t) => {
  const target = scratch(t, "gate-tmp-target-");
  const dir = openGateTmp(target, 1);
  mkdirSync(join(dir, ".tmpLeaked"));
  writeFileSync(join(dir, ".tmpLeaked", "ark.db"), "x");
  writeFileSync(join(dir, ".tmpOther"), "y");
  assert.deepEqual(sweepGateTmp(target, dir), [
    { name: ".tmpLeaked", contents: ["ark.db"] },
    { name: ".tmpOther", contents: [] },
  ]);
  assert.ok(!existsSync(dir));
});

test("non-zero leftovers are reported as a gate failure", (t) => {
  // KOS-270: leftover entries after the test processes exit are a leak, and
  // a leak must be a red gate, not a log line. reportGateTmpSweep returns
  // true when it had to remove anything — the exit handler turns that into
  // a non-zero exit code.
  const target = scratch(t, "gate-tmp-target-");
  const lines = [];
  const log = (line) => lines.push(line);
  assert.equal(reportGateTmpSweep(target, openGateTmp(target, 1), log), false);
  const dirty = openGateTmp(target, 2);
  mkdirSync(join(dirty, ".tmpLeaked"));
  writeFileSync(join(dirty, ".tmpLeaked", "ark.db"), "x");
  assert.equal(reportGateTmpSweep(target, dirty, log), true);
  assert.match(lines.join("\n"), /swept 1 leftover entry/);
  assert.match(lines.join("\n"), /leftover \.tmpLeaked \(ark\.db\)/);
});

test("sweepGateTmp refuses to delete anything outside <target>/gate-tmp", (t) => {
  const target = scratch(t, "gate-tmp-target-");
  const outside = scratch(t, "gate-tmp-outside-");
  assert.throws(() => sweepGateTmp(target, outside), /not under/);
  assert.ok(existsSync(outside));
});

test("aliasGateTmp gives Unix a short link into the run dir and keeps Windows as is", (t) => {
  // mbx binds its cache socket under TMPDIR; the real run dir of a deeply
  // nested checkout overflows SUN_LEN, so Unix gets a short alias instead.
  const target = scratch(t, "gate-tmp-target-");
  const root = scratch(t, "gate-tmp-alias-");
  const dir = openGateTmp(target, 7);
  assert.equal(aliasGateTmp(dir, 7, { platform: "win32", root }).path, dir);
  const alias = aliasGateTmp(dir, 7, { platform: "darwin", root });
  assert.equal(alias.path, join(root, "cortex-gate-7"));
  assert.equal(realpathSync(alias.path), realpathSync(dir));
  writeFileSync(join(alias.path, ".tmpViaAlias"), "x");
  alias.release();
  assert.ok(!existsSync(alias.path));
  assert.deepEqual(sweepGateTmp(target, dir), [{ name: ".tmpViaAlias", contents: [] }]);
});

test("a leak logs the build-tree processes still alive before the sweep", (t) => {
  // KOS-397: on Windows the holder of a leaked ark.db is a process that is
  // still running after nextest returned. The log must name it, and the
  // leak result itself must not change.
  const target = scratch(t, "gate-tmp-target-");
  const dirty = openGateTmp(target, 3);
  mkdirSync(join(dirty, ".tmpLeaked"));
  writeFileSync(join(dirty, ".tmpLeaked", "ark.db"), "x");
  const lines = [];
  let leftoverSeenByListing = false;
  const listProcesses = () => {
    leftoverSeenByListing = existsSync(join(dirty, ".tmpLeaked", "ark.db"));
    return [
      { pid: 4242, ppid: 7, name: "engine.exe", exe: join(target, "debug", "engine.exe"), cmd: "engine.exe" },
    ];
  };
  assert.equal(reportGateTmpSweep(target, dirty, (l) => lines.push(l), { listProcesses }), true);
  assert.ok(leftoverSeenByListing, "the listing runs while the leftovers are still on disk");
  assert.match(lines.join("\n"), /1 live build-tree process at sweep time/);
  assert.match(lines.join("\n"), /pid 4242 ppid 7 engine\.exe/);
  assert.ok(!existsSync(dirty));
});

test("a clean sweep does not list processes", (t) => {
  const target = scratch(t, "gate-tmp-target-");
  let listed = false;
  const listProcesses = () => {
    listed = true;
    return [];
  };
  assert.equal(reportGateTmpSweep(target, openGateTmp(target, 4), () => {}, { listProcesses }), false);
  assert.equal(listed, false);
});

test("a failed process listing is logged and does not change the leak result", (t) => {
  const target = scratch(t, "gate-tmp-target-");
  const dirty = openGateTmp(target, 5);
  mkdirSync(join(dirty, ".tmpLeaked"));
  writeFileSync(join(dirty, ".tmpLeaked", "ark.db"), "x");
  const lines = [];
  const listProcesses = () => {
    throw new Error("powershell unavailable");
  };
  assert.equal(reportGateTmpSweep(target, dirty, (l) => lines.push(l), { listProcesses }), true);
  assert.match(lines.join("\n"), /process listing failed: powershell unavailable/);
  assert.ok(!existsSync(dirty));
});

test("the process listing is empty off Windows", (t) => {
  // The leak is a Windows handle. Other platforms never ask the OS for
  // processes; Windows asks, and may legitimately find none.
  const target = scratch(t, "gate-tmp-target-");
  const rows = listBuildTreeProcesses(target, openGateTmp(target, 6));
  assert.ok(Array.isArray(rows));
  if (process.platform !== "win32") assert.deepEqual(rows, []);
});

test("a leak logs when each leftover file was last written", (t) => {
  // KOS-397: a leftover file has no holder, so its last write is the only clue
  // to the test that left it behind. The log must carry it.
  const target = scratch(t, "gate-tmp-target-");
  const dirty = openGateTmp(target, 8);
  mkdirSync(join(dirty, ".tmpLeaked"));
  writeFileSync(join(dirty, ".tmpLeaked", "ark.db"), "x");
  const lines = [];
  reportGateTmpSweep(target, dirty, (l) => lines.push(l), { listProcesses: () => [] });
  assert.match(lines.join("\n"), /last written \d{4}-\d{2}-\d{2}T[\d:.]+Z \.tmpLeaked[\\/]ark\.db/);
});
