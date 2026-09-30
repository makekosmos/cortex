import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { openGateTmp, sweepGateTmp } from "./gate-tmp.mjs";

test("openGateTmp creates a fresh empty dir under <target>/gate-tmp", () => {
  const target = mkdtempSync(join(tmpdir(), "gate-tmp-target-"));
  const dir = openGateTmp(target, 1234);
  assert.equal(dir, resolve(target, "gate-tmp", "run-1234"));
  assert.ok(existsSync(dir));
  assert.deepEqual(sweepGateTmp(target, dir), []);
  assert.ok(!existsSync(dir));
});

test("sweepGateTmp removes leftover test dirs and reports the count", () => {
  const target = mkdtempSync(join(tmpdir(), "gate-tmp-target-"));
  const dir = openGateTmp(target, 1);
  mkdirSync(join(dir, ".tmpLeaked"));
  writeFileSync(join(dir, ".tmpLeaked", "ark.db"), "x");
  writeFileSync(join(dir, ".tmpOther"), "y");
  assert.deepEqual(sweepGateTmp(target, dir), [".tmpLeaked", ".tmpOther"]);
  assert.ok(!existsSync(dir));
});

test("sweepGateTmp refuses to delete anything outside <target>/gate-tmp", () => {
  const target = mkdtempSync(join(tmpdir(), "gate-tmp-target-"));
  const outside = mkdtempSync(join(tmpdir(), "gate-tmp-outside-"));
  assert.throws(() => sweepGateTmp(target, outside), /not under/);
  assert.ok(existsSync(outside));
});
