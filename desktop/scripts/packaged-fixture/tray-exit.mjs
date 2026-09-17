// tray-exit (KOS-12): real tray menu Exit -> backend 42 -> Desktop closes.
// The Win32 menu automation lives in ./tray-exit.ps1 next to this module.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { processInfo, sameProcessIdentity, stopProcessTree } from "../dev-run-process.mjs";
import {
  DEFAULT_OUT,
  delay,
  engineShutdown,
  ensureDir,
  fixtureEnv,
  iso,
  loadFixture,
  makeCheck,
  processesUnder,
  scenarioNotRun,
  spawnLogged,
  waitFor,
  waitForEngineLock,
  waitPidGone,
  writeJson,
} from "./lib.mjs";

const TRAY_EXIT_PS1 = path.join(path.dirname(fileURLToPath(import.meta.url)), "tray-exit.ps1");

function backendTrayCreated(dataDir) {
  try {
    return fs
      .readdirSync(path.join(dataDir, "logs"))
      .filter((name) => name.startsWith("kepler-backend"))
      .some((name) =>
        fs.readFileSync(path.join(dataDir, "logs", name), "utf8").includes("tray created"),
      );
  } catch {
    return false;
  }
}

async function driveTrayExit(runDir, backendPid) {
  try {
    const stdout = execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        TRAY_EXIT_PS1,
        "-TargetPid",
        String(backendPid),
      ],
      {
        encoding: "utf8",
        windowsHide: true,
        timeout: 45_000,
      },
    );
    fs.writeFileSync(path.join(runDir, `tray-exit-${backendPid}.log`), stdout, "utf8");
  } catch (error) {
    const detail = [error.stdout, error.stderr].filter(Boolean).join("\n");
    fs.writeFileSync(path.join(runDir, `tray-exit-${backendPid}.log`), detail, "utf8");
    throw new Error(detail.trim() || error.message);
  }
}

export async function cmdTrayExit(args) {
  const outDir = path.resolve(args.out ?? DEFAULT_OUT);
  let fixture;
  try {
    fixture = loadFixture(outDir);
  } catch (error) {
    scenarioNotRun(outDir, "tray-exit", error.message);
    return;
  }
  const runDir = path.join(outDir, "runs", "tray-exit");
  fs.rmSync(runDir, { recursive: true, force: true });
  ensureDir(runDir);

  const result = {
    scenario: "tray-exit",
    started_at: iso(),
    status: "FAIL",
    checks: [],
    logs: {},
    legs: {},
    cleanup: {},
  };
  const check = makeCheck(result);

  const engineBackend = fixture.engine.backend;
  const spawned = [];
  const track = (child, label) => {
    if (!child?.pid) return;
    const identity = processInfo(child.pid);
    if (identity) spawned.push({ pid: child.pid, identity, label });
  };
  const processRoots = [runDir, fixture.engine.root, fixture.packaged_root];

  try {
    // --- leg A: bare packaged Engine, tray Exit -> supervisor exit code 42 ---
    {
      const dataDir = path.join(runDir, "engine-data");
      ensureDir(dataDir);
      const engineEnv = fixtureEnv({
        KOSMOS_DATA_DIR: dataDir,
        ARK_CORE_RPC_PATH: fixture.engine.ark,
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
        KOSMOS_TEST_MODE: "1",
        KOSMOS_HEADLESS: "1",
        KEPLER_SKIP_SYNC: "1",
        KEPLER_USAGE_TRACKER: "0",
        KEPLER_INSTANCE: "kos58-tray",
      });
      if (fixture.engine.tray) engineEnv.KOSMOS_TRAY_ICON = fixture.engine.tray;
      const engine = spawnLogged(engineBackend, ["--start"], {
        env: engineEnv,
        logFile: path.join(runDir, "engine-tray.log"),
      });
      track(engine.child, "engine");
      result.logs.engine = path.join(runDir, "engine-tray.log");
      try {
        await waitFor(() => backendTrayCreated(dataDir) || undefined, "backend tray", 30_000);
        check("engine leg: tray created", true);
        const engineLock = await waitForEngineLock(dataDir, 10_000).catch(() => null);
        await driveTrayExit(runDir, engineLock?.pid ?? engine.child.pid);
        const { code } = await Promise.race([
          engine.exited,
          delay(20_000).then(() => ({ code: "timeout" })),
        ]);
        check("engine leg: backend exit code 42 after tray Exit", code === 42, `exit=${code}`);
        result.legs.engine = { status: code === 42 ? "PASS" : "FAIL", exitCode: code };
      } catch (error) {
        check("engine leg", false, error.message);
        result.legs.engine = { status: "FAIL", error: error.message };
      } finally {
        engineShutdown(engineBackend, dataDir);
        if (engine.child?.pid) await waitPidGone(engine.child.pid, "Engine").catch(() => {});
      }
    }

    // --- leg B: packaged Kosmos.exe -> backend 42 -> Desktop closes cleanly ---
    {
      const dataDir = path.join(runDir, "desktop-data");
      // Test slot derives userData as <KOSMOS_DATA_DIR>/userdata — the boot
      // self-check rejects a --user-data-dir that doesn't match it.
      const userData = path.join(dataDir, "userdata");
      ensureDir(dataDir);
      // Shells <=0.9.36 don't pass KOSMOS_TRAY_ICON to the backend and the
      // fixture Engine dir has no tray.ico (manifest gates reject extra
      // files), so point the backend at the packaged resources icon — the
      // same path newer packaged shells resolve themselves.
      const shellEnv = fixtureEnv({
        APPDATA: path.join(runDir, "appdata"),
        LOCALAPPDATA: path.join(runDir, "localappdata"),
        KOSMOS_DATA_DIR: dataDir,
        KOSMOS_ENGINE_ROOT: fixture.engine.root,
        KEPLER_INSTANCE: "test-kos58-tray",
        KOSMOS_TEST_MODE: "1",
        KOSMOS_HEADLESS: "1",
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
        KEPLER_SKIP_SYNC: "1",
        KEPLER_USAGE_TRACKER: "0",
      });
      if (fixture.engine.tray) shellEnv.KOSMOS_TRAY_ICON = fixture.engine.tray;
      const shell = spawnLogged(
        path.join(fixture.packaged_root, "Kosmos.exe"),
        [`--user-data-dir=${userData}`],
        {
          env: shellEnv,
          cwd: fixture.packaged_root,
          logFile: path.join(runDir, "desktop-tray.log"),
        },
      );
      track(shell.child, "desktop");
      result.logs.desktop = path.join(runDir, "desktop-tray.log");
      try {
        const shellLock = await waitForEngineLock(dataDir, 60_000);
        await waitFor(() => backendTrayCreated(dataDir) || undefined, "backend tray", 30_000);
        check("desktop leg: packaged backend + tray up", true);
        await driveTrayExit(runDir, shellLock.pid);
        const { code } = await Promise.race([
          shell.exited,
          delay(30_000).then(() => ({ code: "timeout" })),
        ]);
        check(
          "desktop leg: Kosmos.exe closed after backend exit 42",
          code !== "timeout",
          `exit=${code}`,
        );
        result.legs.desktop = {
          status: code !== "timeout" ? "PASS" : "FAIL",
          exitCode: code,
        };
      } catch (error) {
        check("desktop leg", false, error.message);
        result.legs.desktop = { status: "FAIL", error: error.message };
      } finally {
        engineShutdown(engineBackend, dataDir);
        for (const item of spawned.filter((s) => s.label === "desktop")) {
          const current = processInfo(item.pid);
          if (current && sameProcessIdentity(current, item.identity)) {
            try {
              stopProcessTree(
                item.pid,
                item.identity.startTime,
                item.identity.commandLine,
                "desktop",
              );
            } catch {}
          }
        }
      }
    }
  } finally {
    // Drain: supervisor children can outlive tracked parent pids briefly.
    const deadline = Date.now() + 15_000;
    let leftovers = processesUnder(processRoots);
    while (leftovers.length && Date.now() < deadline) {
      for (const leftover of leftovers) {
        const current = processInfo(leftover.ProcessId);
        if (current) {
          try {
            stopProcessTree(leftover.ProcessId, current.startTime, current.commandLine, "fixture");
          } catch {}
        }
      }
      await delay(300);
      leftovers = processesUnder(processRoots);
    }
    result.cleanup = {
      leftover_processes: leftovers,
      roots: processRoots,
      engine_locks_removed: [
        path.join(runDir, "engine-data", "engine.lock.json"),
        path.join(runDir, "desktop-data", "engine.lock.json"),
      ].every((file) => !fs.existsSync(file)),
    };
    check(
      "cleanup: no leftover fixture processes",
      leftovers.length === 0,
      leftovers.map((p) => p.ExecutablePath).join("; ") || undefined,
    );
    check("cleanup: engine locks removed", result.cleanup.engine_locks_removed);
    result.status = result.checks.every((item) => item.status === "pass") ? "PASS" : "FAIL";
    result.finished_at = iso();
    writeJson(path.join(runDir, "result.json"), result);
  }
  console.log(JSON.stringify({ scenario: "tray-exit", status: result.status, runDir }));
  if (result.status !== "PASS") process.exitCode = 1;
}
