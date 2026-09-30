import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { openGateTmp, reportGateTmpSweep, sweepGateTmp } from "./gate-tmp.mjs";

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
