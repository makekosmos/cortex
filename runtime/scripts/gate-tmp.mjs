import { lstatSync, mkdirSync, readdirSync, rmSync, symlinkSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join, resolve, sep } from "node:path";

// KOS-270: engine/ark-core tests must never write to the user's %TEMP%.
// `openGateTmp` creates a fresh per-run scratch directory inside the cargo
// target dir; the gate injects it as TMP/TEMP/TMPDIR for every cargo
// invocation so tempfile::tempdir() and friends land there. After the test
// processes have exited — when every leaked handle is closed — the gate
// sweeps the directory. Any leftover entry is a leaked fixture and fails the
// gate, so a leak is a red gate instead of silent accumulation.

export function openGateTmp(targetDir, runId) {
  const dir = resolve(targetDir, "gate-tmp", `run-${runId}`);
  mkdirSync(dir, { recursive: true });
  return dir;
}

// Unix sockets cap their path at ~104 bytes (SUN_LEN), and tools that open
// one under TMPDIR — mbx's `mbx-session-*/cache-agent.sock` — overflow it
// once the checkout path is long enough. On Unix the gate therefore hands out
// a short `/tmp/cortex-gate-<runId>` symlink to the real run dir: the bytes
// still land in the target dir (KOS-270), only the path is short. Windows
// keeps the real dir. `release` removes just the link, before the sweep.
export function aliasGateTmp(dir, runId, { platform = process.platform, root = "/tmp" } = {}) {
  if (platform === "win32") return { path: dir, release() {} };
  const link = join(root, `cortex-gate-${runId}`);
  rmSync(link, { force: true });
  // Tests can exercise the Unix branch on a Windows host. A junction keeps
  // that fixture unprivileged; production Windows still returns above.
  symlinkSync(dir, link, process.platform === "win32" ? "junction" : "dir");
  return {
    path: link,
    release() {
      try {
        if (lstatSync(link).isSymbolicLink()) rmSync(link);
      } catch {
        // Already gone.
      }
    },
  };
}

// Removes `dir` and returns its leftover entries as `{ name, contents }`
// (empty list when the tests cleaned up after themselves). Contents are
// captured before removal so a leak stays attributable to its test.
// `dir` must live under `targetDir`: this function deletes recursively, and
// refusing anything outside the build tree is the only guard against a bad
// caller. `onLeftover` runs once the leftovers are listed and before anything
// is removed, so a diagnostic can see what still holds them.
export function sweepGateTmp(targetDir, dir, { onLeftover } = {}) {
  const root = resolve(targetDir, "gate-tmp") + sep;
  if (!resolve(dir).startsWith(root)) {
    throw new Error(`refusing to sweep ${dir}: not under ${root}`);
  }
  const leftover = readdirSync(dir).map((name) => {
    let contents;
    try {
      contents = readdirSync(join(dir, name));
    } catch {
      contents = [];
    }
    return { name, contents };
  });
  if (leftover.length > 0) onLeftover?.(leftover);
  rmSync(dir, { recursive: true, force: true });
  return leftover;
}

// KOS-397: a leak is a process that still holds a file in the run dir after
// nextest has returned. Windows keeps the handle until that process exits, so
// the processes still running from the build tree (test binaries and the
// engine children they start) are the candidates. This lists them by image
// path or by a command line that names the run dir. Other platforms have no
// such leak, so they list nothing. The list is for the log only: it never
// changes what the gate removes or whether it fails.
export function listBuildTreeProcesses(targetDir, dir) {
  if (process.platform !== "win32") return [];
  const quote = (value) => `'${value.replaceAll("'", "''")}'`;
  const script = [
    `$tree = ${quote(resolve(targetDir))}`,
    `$run = ${quote(dir)}`,
    "Get-CimInstance Win32_Process |",
    "  Where-Object { ($_.ExecutablePath -and $_.ExecutablePath.StartsWith($tree, [StringComparison]::OrdinalIgnoreCase)) -or ($_.CommandLine -and $_.CommandLine.Contains($run)) } |",
    "  ForEach-Object { [pscustomobject]@{ pid = $_.ProcessId; ppid = $_.ParentProcessId; name = $_.Name; exe = $_.ExecutablePath; cmd = $_.CommandLine } } |",
    "  ConvertTo-Json -Compress",
  ].join("\n");
  // -EncodedCommand keeps the script intact on the command line, whatever
  // quoting the path needs.
  const encoded = Buffer.from(script, "utf16le").toString("base64");
  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-EncodedCommand", encoded],
    { encoding: "utf8", timeout: 20_000 },
  );
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr.trim() || `exit ${result.status}`);
  const out = result.stdout.trim();
  if (!out) return [];
  // An empty pipeline can serialize as `null`; that is "no processes", not a row.
  const rows = JSON.parse(out);
  return (Array.isArray(rows) ? rows : [rows]).filter(Boolean);
}

// Sweeps and logs the result. Returns true when leftovers existed — the gate
// treats that as a leak and fails. A leak also logs the build-tree processes
// still alive at that point, so the log names the holder. A failed listing is
// logged and never changes the result.
export function reportGateTmpSweep(
  targetDir,
  dir,
  log = console.log,
  { listProcesses = listBuildTreeProcesses } = {},
) {
  const leftover = sweepGateTmp(targetDir, dir, {
    onLeftover() {
      try {
        logLiveProcesses(listProcesses(targetDir, dir), log);
      } catch (error) {
        log(`gate tmp: process listing failed: ${error.message}`);
      }
    },
  });
  log(
    `gate tmp: swept ${leftover.length} leftover entr${leftover.length === 1 ? "y" : "ies"} under ${dir}`,
  );
  for (const { name, contents } of leftover.slice(0, 20)) {
    log(`gate tmp:   leftover ${name} (${contents.join(",") || "-"})`);
  }
  return leftover.length > 0;
}

function logLiveProcesses(rows, log) {
  log(`gate tmp: ${rows.length} live build-tree process${rows.length === 1 ? "" : "es"} at sweep time`);
  for (const row of rows.slice(0, 20)) {
    const cmd = String(row.cmd ?? "-").slice(0, 300);
    log(`gate tmp:   pid ${row.pid} ppid ${row.ppid} ${row.name} exe=${row.exe ?? "-"} cmd=${cmd}`);
  }
}
