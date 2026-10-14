// KOS-306 / KOS-399: static Defender gate for the Windows installer.
//
// `MpCmdRun -Scan -ScanType 3 -File` is a read-only custom scan. It catches
// static signatures and the local ML model, not the cloud first-sight verdict.
//
// MpCmdRun exits non-zero both when it finds a threat and when the scanner
// itself cannot run (hosted runners sometimes have the Defender service
// stopped: "failed with hr = 0x800106BA"). Those are different outcomes:
// - "detection": blocks the build, always.
// - "unavailable": retried (after trying to start WinDefend); if it stays
//   unavailable the build continues with a loud, clearly labelled warning,
//   because the scan proves nothing either way. Set
//   MUNDUS_DEFENDER_REQUIRED=1 to fail instead.
// - "error": any other non-zero exit we cannot classify; blocks the build
//   with its own message so it is never reported as a virus.

import { spawnSync } from "node:child_process";

/** MpCmdRun HRESULTs that mean "the scanner did not run", not "threat". */
export const UNAVAILABLE_HRESULTS = [
  "0x800106ba", // service not running / RPC to engine failed
  "0x80070422", // service disabled
  "0x8050800c", // engine unavailable
  "0x80508023", // engine failed to initialize
];

/**
 * Classify one MpCmdRun run from its exit status and combined output.
 * @returns {{kind: "clean"|"detection"|"unavailable"|"error", detail: string}}
 */
export function classifyMpCmdRun(status, output = "") {
  const text = String(output);
  const lower = text.toLowerCase();
  const threats = /found\s+(\d+)\s+threats?/i.exec(text);
  if (status === 2 || (threats && Number(threats[1]) > 0) || /threat\s+information/i.test(text))
    return { kind: "detection", detail: threats ? `${threats[1]} threat(s)` : `exit ${status}` };
  if (status === 0) return { kind: "clean", detail: "no threats" };
  const hr = UNAVAILABLE_HRESULTS.find((code) => lower.includes(code));
  if (hr) return { kind: "unavailable", detail: `hr ${hr}` };
  if (/service.*(not (running|started)|disabled)/i.test(text))
    return { kind: "unavailable", detail: "Defender service not running" };
  return { kind: "error", detail: `exit ${status ?? "(signal)"}` };
}

function runScan(mpCmdRun, outFile) {
  const r = spawnSync(
    mpCmdRun,
    ["-Scan", "-ScanType", "3", "-File", outFile, "-DisableRemediation"],
    { encoding: "utf8", windowsHide: true },
  );
  const output = `${r.stdout ?? ""}${r.stderr ?? ""}`;
  if (output) process.stdout.write(output.endsWith("\n") ? output : `${output}\n`);
  return classifyMpCmdRun(r.status, output);
}

function sleepMs(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

/**
 * Run the gate. `scan` and `startService` are injectable for tests.
 * Returns the final classification; throws on detection/error, and on
 * unavailable when `required`.
 */
export function runDefenderGate({
  outFile,
  mpCmdRun,
  attempts = 3,
  delayMs = 15000,
  required = process.env.MUNDUS_DEFENDER_REQUIRED === "1",
  scan = () => runScan(mpCmdRun, outFile),
  startService = () =>
    spawnSync("sc.exe", ["start", "WinDefend"], { stdio: "ignore", windowsHide: true }),
  wait = sleepMs,
  log = console.log,
}) {
  let result;
  for (let i = 1; i <= attempts; i++) {
    result = scan();
    if (result.kind === "detection")
      throw new Error(`Defender scan reported a detection in ${outFile} (${result.detail})`);
    if (result.kind === "error")
      throw new Error(`Defender scan failed for ${outFile} (${result.detail}); not a detection`);
    if (result.kind === "clean") return result;
    log(`Defender scanner unavailable (${result.detail}), attempt ${i}/${attempts}`);
    if (i < attempts) {
      startService();
      wait(delayMs);
    }
  }
  const msg = `Defender scanner unavailable after ${attempts} attempts (${result.detail}); ${outFile} was NOT scanned`;
  if (required) throw new Error(msg);
  log(`::warning::${msg}`);
  return result;
}
