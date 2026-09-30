import { mkdirSync, readdirSync, rmSync } from "node:fs";
import { join, resolve, sep } from "node:path";

// KOS-270: engine/ark-core tests must never write to the user's %TEMP%.
// `openGateTmp` creates a fresh per-run scratch directory inside the cargo
// target dir; the gate injects it as TMP/TEMP/TMPDIR for every cargo
// invocation so tempfile::tempdir() and friends land there. After the test
// processes have exited — when every leaked handle is closed — the gate
// sweeps the directory and reports how many entries had to be removed, so a
// leaked fixture stays visible in the log instead of accumulating silently.

export function openGateTmp(targetDir, runId) {
  const dir = resolve(targetDir, "gate-tmp", `run-${runId}`);
  mkdirSync(dir, { recursive: true });
  return dir;
}

// Returns the leftover entry names (empty list when the tests cleaned up
// after themselves). `dir` must live under `targetDir`: this function
// deletes recursively, and refusing anything outside the build tree is the
// only guard against a bad caller.
export function sweepGateTmp(targetDir, dir) {
  const root = resolve(targetDir, "gate-tmp") + sep;
  if (!resolve(dir).startsWith(root)) {
    throw new Error(`refusing to sweep ${dir}: not under ${root}`);
  }
  const leftover = readdirSync(dir);
  rmSync(dir, { recursive: true, force: true });
  return leftover;
}

// Reports what the sweep removed. Returns true when leftovers existed —
// the gate treats that as a leak and fails: a leftover entry means a test
// finished with its fixture still on disk (KOS-270).
export function reportGateTmpSweep(targetDir, dir, log = console.log) {
  const root = resolve(targetDir, "gate-tmp") + sep;
  if (!resolve(dir).startsWith(root)) {
    throw new Error(`refusing to sweep ${dir}: not under ${root}`);
  }
  const removed = readdirSync(dir);
  // Capture entry contents before removal so leaks stay attributable.
  const contents = removed.map((name) => {
    try {
      return readdirSync(join(dir, name)).join(",");
    } catch {
      return "-";
    }
  });
  rmSync(dir, { recursive: true, force: true });
  log(
    `gate tmp: swept ${removed.length} leftover entr${removed.length === 1 ? "y" : "ies"} under ${dir}`,
  );
  for (const [i, name] of removed.slice(0, 20).entries()) {
    log(`gate tmp:   leftover ${name} (${contents[i]})`);
  }
  return removed.length > 0;

}
