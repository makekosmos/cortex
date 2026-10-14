import assert from "node:assert/strict";
import { test } from "node:test";
import { classifyMpCmdRun, runDefenderGate } from "./defender-gate.mjs";

const CLEAN = `Scan starting...\nScan finished.\nScanning C:\\r\\Mundus-Setup.exe found no threats.\n`;
const DETECTION = `Scan starting...\nScan finished.\nScanning C:\\r\\Mundus-Setup.exe found 1 threats.\n\n<===========================LIST OF DETECTED THREATS==========================>\n----------------------------- Threat information ------------------------------\nThreat                  : Trojan:Win32/Wacatac.B!ml\n`;
const UNAVAILABLE = `Scan starting...\nCmdTool: Failed with hr = 0x800106BA. Check C:\\Users\\RUNNER~1\\AppData\\Local\\Temp\\MpCmdRun.log for more information\n`;

test("classifies clean, detection and unavailable MpCmdRun output", () => {
  assert.equal(classifyMpCmdRun(0, CLEAN).kind, "clean");
  assert.equal(classifyMpCmdRun(2, DETECTION).kind, "detection");
  assert.equal(classifyMpCmdRun(2, "").kind, "detection");
  assert.equal(classifyMpCmdRun(1, DETECTION).kind, "detection");
  const u = classifyMpCmdRun(2147549370 % 256, UNAVAILABLE);
  assert.equal(u.kind, "unavailable");
  assert.match(u.detail, /0x800106ba/);
  assert.equal(classifyMpCmdRun(-2147418438, UNAVAILABLE).kind, "unavailable");
  assert.equal(classifyMpCmdRun(5, "something odd").kind, "error");
});

const quiet = { wait: () => {}, startService: () => {}, log: () => {}, outFile: "x.exe" };

test("detection always blocks", () => {
  assert.throws(
    () => runDefenderGate({ ...quiet, scan: () => classifyMpCmdRun(2, DETECTION) }),
    /detection/,
  );
});

test("unavailable retries, then succeeds when the scanner comes back", () => {
  const seq = [classifyMpCmdRun(-1, UNAVAILABLE), classifyMpCmdRun(0, CLEAN)];
  let started = 0;
  const r = runDefenderGate({ ...quiet, scan: () => seq.shift(), startService: () => started++ });
  assert.equal(r.kind, "clean");
  assert.equal(started, 1);
});

test("persistently unavailable warns (not a virus) or fails when required", () => {
  const logs = [];
  const scan = () => classifyMpCmdRun(-1, UNAVAILABLE);
  const r = runDefenderGate({ ...quiet, scan, required: false, log: (m) => logs.push(m) });
  assert.equal(r.kind, "unavailable");
  assert.ok(logs.some((m) => m.startsWith("::warning::") && /unavailable/.test(m)));
  assert.ok(!logs.some((m) => /detection/.test(m)));
  assert.throws(
    () => runDefenderGate({ ...quiet, scan, required: true }),
    /unavailable.*NOT scanned/,
  );
});

test("unclassified failure is reported as a scan failure, not a detection", () => {
  assert.throws(
    () => runDefenderGate({ ...quiet, scan: () => classifyMpCmdRun(5, "odd") }),
    (e) => /not a detection/.test(e.message),
  );
});
