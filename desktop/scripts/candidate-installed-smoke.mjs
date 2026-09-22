#!/usr/bin/env node
import { execFileSync, spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createServer } from "node:net";
import { installEngineArchive, resolveInstalledEngine } from "./engine-distribution.mjs";

const normalMode = process.argv.includes("--normal");
const dictationNativeNegative = process.argv.includes("--dictation-native-negative");
const candidateArg = process.argv
  .slice(2)
  .find((arg) => !["--normal", "--dictation-native-negative"].includes(arg));
if (normalMode && !candidateArg) throw new Error("--normal requires an installed root path");
const candidate = path.resolve(candidateArg ?? "release/win-unpacked");
const resources = path.join(candidate, "resources");
const engineArchive = path.join(resources, "Kosmos Engine.zip");
const engineManifest = path.join(resources, "engine-manifest.json");
const managerExe = path.join(resources, "components", "manager", "Kosmos Manager.exe");
const hostExe = path.join(resources, "components", "host", "Kosmos Package Host.exe");
for (const file of [engineArchive, engineManifest, managerExe, hostExe]) {
  if (!fs.existsSync(file)) throw new Error(`missing candidate artifact: ${path.basename(file)}`);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const waitFor = async (read, label, timeout = 30_000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await read();
    if (value !== undefined) return value;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${label}`);
};
const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
const waitGone = async (pid, label) => {
  await waitFor(() => (alive(pid) ? undefined : true), `${label} exit`, 10_000);
};
const port = async () => {
  const server = createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const value = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return value;
};
const launchElectronDebug = async (executable, userData, args = []) => {
  const debugPort = await port();
  const child = spawn(
    executable,
    [`--remote-debugging-port=${debugPort}`, `--user-data-dir=${userData}`, ...args],
    {
      env,
      stdio: "ignore",
      windowsHide: !normalMode,
    },
  );
  const targets = await waitFor(async () => {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`, {
        signal: AbortSignal.timeout(1_000),
      });
      if (!response.ok) return undefined;
      const value = await response.json();
      return Array.isArray(value) && value.some((item) => item.type === "page") ? value : undefined;
    } catch {
      return undefined;
    }
  }, "Electron DevTools endpoint");
  return { child, targets, debugPort };
};
const closeCdpTarget = async (debugPort, target) => {
  const response = await fetch(
    `http://127.0.0.1:${debugPort}/json/close/${encodeURIComponent(target.id)}`,
    { signal: AbortSignal.timeout(5_000) },
  );
  if (!response.ok) throw new Error(`CDP target close failed: HTTP ${response.status}`);
};
const stop = async (child, label) => {
  if (!child?.pid || !alive(child.pid)) return;
  let killError;
  try {
    child.kill("SIGKILL");
  } catch (error) {
    killError = error instanceof Error ? error.message : String(error);
  }
  if (alive(child.pid)) {
    try {
      process.kill(child.pid, "SIGKILL");
    } catch (error) {
      killError ??= error instanceof Error ? error.message : String(error);
    }
  }
  if (alive(child.pid)) {
    try {
      await waitGone(child.pid, label);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      throw new Error(
        `${label} exit timeout after native kill${killError ? ` (${killError})` : ""}: ${detail}`,
      );
    }
  }
};
const windowInventory = (pids) => {
  const output = execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-Command",
      "$ids = $env:KOSMOS_SMOKE_PIDS -split ','; @($ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue } | Select-Object Id,ProcessName,MainWindowHandle) | ConvertTo-Json -Compress",
    ],
    {
      encoding: "utf8",
      windowsHide: true,
      env: { ...process.env, KOSMOS_SMOKE_PIDS: pids.join(",") },
    },
  ).trim();
  if (!output || output === "[]") return [];
  const parsed = JSON.parse(output);
  return (Array.isArray(parsed) ? parsed : [parsed]).map((item) => ({
    pid: item.Id,
    name: item.ProcessName,
    visibleWindow: Number(item.MainWindowHandle) !== 0,
  }));
};
const consoleInventory = () => {
  const output = execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-Command",
      "@(Get-Process -Name powershell,pwsh,cmd,conhost -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,MainWindowHandle) | ConvertTo-Json -Compress",
    ],
    { encoding: "utf8", windowsHide: true },
  ).trim();
  if (!output || output === "[]") return [];
  const parsed = JSON.parse(output);
  return (Array.isArray(parsed) ? parsed : [parsed]).map((item) => ({
    pid: item.Id,
    name: item.ProcessName,
    visibleWindow: Number(item.MainWindowHandle) !== 0,
  }));
};
const consoleDelta = (before, after) =>
  after.filter((item) => !before.some((previous) => previous.pid === item.pid));
const kosmosProcessInventory = () => {
  const output = execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-Command",
      "@(Get-CimInstance Win32_Process | Where-Object { $_.Name -like 'Kosmos*' } | Select-Object ProcessId,ParentProcessId,Name,WindowHandle) | ConvertTo-Json -Compress",
    ],
    { encoding: "utf8", windowsHide: true },
  ).trim();
  if (!output || output === "[]") return [];
  const parsed = JSON.parse(output);
  return (Array.isArray(parsed) ? parsed : [parsed]).map((item) => ({
    pid: item.ProcessId,
    parentPid: item.ParentProcessId,
    name: item.Name,
    visibleWindow: Number(item.WindowHandle) !== 0,
  }));
};
const kosmosProcessDelta = (before, after) =>
  after.filter((item) => !before.some((previous) => previous.pid === item.pid));
const redactedTarget = (item) => {
  let route = "redacted";
  try {
    const url = new URL(String(item.url));
    route = url.pathname.endsWith("/index.html")
      ? "index.html"
      : url.pathname.replace(
          /^\/v1\/apps\/assets\/[^/]+\/dist\/index\.html$/,
          "/v1/apps/assets/<package>/dist/index.html",
        );
  } catch {}
  return { id: item.id ?? null, type: item.type, route };
};
const shutdownRuntime = () =>
  new Promise((resolve, reject) => {
    const child = spawn(runtime, ["--shutdown"], {
      env,
      stdio: "ignore",
      windowsHide: true,
    });
    let settled = false;
    let timer;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (error) reject(error);
      else resolve();
    };
    timer = setTimeout(() => {
      try {
        child.kill("SIGKILL");
      } catch {}
      finish(new Error("Runtime --shutdown timed out"));
    }, 10_000);
    child.once("error", (error) => finish(error));
    child.once("exit", (code, signal) => {
      if (code === 0) finish();
      else finish(new Error(`Runtime --shutdown exited ${code ?? signal ?? "unknown"}`));
    });
  });
const terminate = async (child, label) => {
  if (!child?.pid) return;
  let shutdownError;
  try {
    await shutdownRuntime();
  } catch (error) {
    shutdownError = error instanceof Error ? error.message : String(error);
  }
  if (!shutdownError && alive(child.pid)) {
    await waitFor(
      () => (alive(child.pid) ? undefined : true),
      `${label} graceful exit`,
      5_000,
    ).catch(() => undefined);
  }
  let killError;
  if (alive(child.pid)) {
    try {
      child.kill("SIGKILL");
    } catch (error) {
      killError = error instanceof Error ? error.message : String(error);
    }
  }
  if (alive(child.pid)) {
    try {
      process.kill(child.pid, "SIGKILL");
    } catch (error) {
      killError ??= error instanceof Error ? error.message : String(error);
    }
  }
  if (alive(child.pid)) {
    try {
      await waitGone(child.pid, label);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      throw new Error(
        `${label} exit timeout after shutdown/native kill${killError ? ` (${killError})` : ""}: ${detail}`,
      );
    }
  }
  if (shutdownError) throw new Error(`${label} shutdown failed: ${shutdownError}`);
};
const expect = (condition, message) => {
  if (!condition) throw new Error(message);
};

const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-candidate-smoke-"));
const dataDir = path.join(root, "data");
const env = {
  ...process.env,
  APPDATA: path.join(root, "appdata"),
  LOCALAPPDATA: path.join(root, "localappdata"),
  KOSMOS_DATA_DIR: dataDir,
  KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
  KEPLER_SKIP_SYNC: "1",
  KEPLER_USAGE_TRACKER: "0",
};
if (normalMode) {
  delete env.KOSMOS_HEADLESS;
  delete env.KOSMOS_TEST_MODE;
} else {
  env.KOSMOS_HEADLESS = "1";
  env.KOSMOS_TEST_MODE = "1";
}
const installedEngineRoot = path.join(env.LOCALAPPDATA, "Kosmos", "Engine");
installEngineArchive(
  engineArchive,
  JSON.parse(fs.readFileSync(engineManifest, "utf8")),
  installedEngineRoot,
);
const installedEngine = resolveInstalledEngine(installedEngineRoot);
const runtime = installedEngine.backend;
const ark = installedEngine.ark;
env.ARK_CORE_RPC_PATH = ark;
const cleanupRoot = () => {
  try {
    fs.rmSync(root, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 200,
    });
    return;
  } catch {}
  execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-Command",
      "Start-Sleep -Seconds 1; $root = $env:KOSMOS_SMOKE_ROOT; if (-not $root.StartsWith((Resolve-Path $env:TEMP).Path + '\\kosmos-candidate-smoke-')) { throw 'unexpected cleanup target' }; Remove-Item -LiteralPath $root -Recurse -Force",
    ],
    {
      stdio: "ignore",
      windowsHide: true,
      env: { ...process.env, KOSMOS_SMOKE_ROOT: root },
    },
  );
};
let engine;
let manager;
let host;
let managerPid;
let hostPid;
let summary;
const before = {
  processes: [],
  windows: [],
  consoles: consoleInventory(),
  kosmosProcesses: kosmosProcessInventory(),
};
try {
  engine = spawn(runtime, normalMode ? ["--start"] : [], {
    env,
    stdio: "ignore",
    windowsHide: true,
  });
  const lock = await waitFor(() => {
    try {
      return JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8"));
    } catch {
      return undefined;
    }
  }, "Runtime lock");
  const rpc = async (operation, params = {}) => {
    const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${lock.auth_token}`,
        "Content-Type": "application/json",
        "X-Kosmos-Api-Version": "1.0.0",
        "X-Kosmos-Client-Class": "candidate-smoke",
        "X-Kosmos-Client-Version": "1.0.0",
        "X-Kosmos-Client-Pid": String(process.pid),
      },
      body: JSON.stringify({ operation, _req_id: randomUUID(), ...params }),
      signal: AbortSignal.timeout(30_000),
    });
    return response.json();
  };
  const rpcEventually = (operation, params = {}) =>
    waitFor(async () => {
      const result = await rpc(operation, params);
      return result.ok ? result : undefined;
    }, operation);

  // components/manager is the manager-gpui exe (KOS-134): there is no CDP
  // endpoint, so launch coverage is process liveness plus — in normal mode —
  // a visible window. Offscreen placement keeps the headless leg invisible.
  manager = {
    child: spawn(managerExe, [], {
      env: normalMode ? env : { ...env, MANAGER_GPUI_OFFSCREEN: "1" },
      stdio: "ignore",
      windowsHide: !normalMode,
    }),
  };
  managerPid = manager.child.pid;
  await sleep(2_000);
  expect(alive(managerPid), "Manager (GPUI) exited during launch");
  if (normalMode)
    await waitFor(
      () => (windowInventory([managerPid]).some((item) => item.visibleWindow) ? true : undefined),
      "Manager window",
    );
  const refresh = await rpcEventually("packages.refresh_catalog");
  expect(refresh.ok, `Manager did not refresh the default catalog: ${JSON.stringify(refresh)}`);
  const trust = await rpc("packages.trust_status");
  expect(
    trust.ok &&
      trust.data?.trust?.configured === true &&
      !trust.data?.trust?.fault_code &&
      trust.data?.catalog?.sequence >= 2,
    "Manager trust is not usable",
  );
  const catalog = await rpc("packages.list", { kind: "app" });
  const shell = catalog.ok && catalog.data.catalog.find((item) => item.id === "com.kosmos.shell");
  expect(shell, "default catalog does not contain com.kosmos.shell");
  for (const id of ["com.kosmos.memoria", "com.kosmos.agenda"]) {
    expect(
      catalog.data.catalog.some((item) => item.id === id),
      `default catalog lacks ${id}`,
    );
  }
  const bridgeCatalog = await rpc("packages.list", { kind: "bridge" });
  const bridge =
    bridgeCatalog.ok &&
    bridgeCatalog.data.catalog.find((item) => item.id === "ark-markdown-bridge");
  expect(bridge, "default catalog does not contain ark-markdown-bridge");
  const storeRefresh = await rpc("store.refresh");
  const storeIds = new Set(storeRefresh.data?.listings?.map((item) => item.id));
  expect(
    storeRefresh.ok &&
      storeRefresh.data?.state === "fresh" &&
      storeRefresh.data?.sequence >= 3 &&
      [
        "com.kosmos.shell",
        "com.kosmos.memoria",
        "com.kosmos.agenda",
        "ark-markdown-bridge",
        "external.obsidian",
      ].every((id) => storeIds.has(id)),
    `Manager did not load Store catalog 3+: ${JSON.stringify(storeRefresh)}`,
  );
  const storeShell = storeRefresh.data.listings.find((item) => item.id === "com.kosmos.shell");
  expect(
    storeShell?.distribution?.version === shell.version,
    "Store and Package Index disagree on the Shell version",
  );
  for (const item of [...catalog.data.catalog, bridge]) {
    const result = await rpcEventually("packages.install", { id: item.id, version: item.version });
    expect(result.ok, `Manager failed to install ${item.id}: ${JSON.stringify(result)}`);
  }
  const shellVersion = shell.version;
  for (const [operation, params] of [
    ["packages.set_enabled", { id: "com.kosmos.shell", version: shellVersion, enabled: true }],
  ]) {
    const result = await rpc(operation, params);
    expect(result.ok, "Manager package mutation failed");
  }
  const installed = await rpc("packages.list", { kind: "app" });
  expect(
    installed.ok &&
      ["com.kosmos.shell", "com.kosmos.memoria", "com.kosmos.agenda"].every((id) =>
        installed.data.packages.some((item) => item.id === id),
      ) &&
      installed.data.packages.some((item) => item.id === "com.kosmos.shell" && item.enabled),
    "first-party apps were not installed or Shell was not enabled",
  );
  const installedBridge = await rpc("packages.list", { kind: "bridge" });
  expect(
    installedBridge.ok &&
      installedBridge.data.packages.some((item) => item.id === "ark-markdown-bridge"),
    "ARK Markdown Bridge was not installed",
  );

  const hostUserData = path.join(root, "host-user-data");
  const hostSettings = await rpc("engine.settings.get");
  const hostWarmTimeoutSeconds = hostSettings.data?.desktop_host?.warm_timeout_seconds;
  let hostWarmReuse = null;
  expect(
    hostSettings.ok && (hostWarmTimeoutSeconds === 0 || hostWarmTimeoutSeconds === 300),
    "Engine desktop Host warm timeout must be 0 or 300 seconds",
  );
  host = await launchElectronDebug(hostExe, hostUserData, ["--open-app=com.kosmos.shell"]);
  hostPid = host.child.pid;
  const windows = windowInventory([engine.pid, managerPid, hostPid]);
  const shellTargets = host.targets.filter((item) => {
    try {
      const url = new URL(String(item.url));
      return (
        item.type === "page" &&
        url.hostname === "127.0.0.1" &&
        Number(url.port) === lock.http_port &&
        /^\/v1\/apps\/assets\/[^/]+\/dist\/index\.html$/.test(url.pathname)
      );
    } catch {
      return false;
    }
  });
  if (normalMode) expect(shellTargets.length === 1, "Host must expose exactly one Shell app page");
  const runtimeWindow = windows.find((item) => item.pid === engine.pid);
  expect(
    normalMode
      ? runtimeWindow && !runtimeWindow.visibleWindow
      : // The offscreen GPUI Manager owns a real HWND while parked at -20000.
        windows.every((item) => !item.visibleWindow || item.pid === managerPid),
    normalMode
      ? "Runtime exposed a visible Win32 window"
      : "headless candidate exposed a visible Win32 window",
  );
  expect(shellTargets.length >= 1, "Host did not open Shell");
  const repeat = spawn(
    hostExe,
    [`--user-data-dir=${hostUserData}`, "--open-app=com.kosmos.shell"],
    {
      env,
      stdio: "ignore",
      windowsHide: true,
    },
  );
  const repeatExit = await new Promise((resolve, reject) => {
    repeat.once("error", reject);
    repeat.once("exit", resolve);
  });
  expect(repeatExit === 0, "repeat Host invocation did not hand off cleanly");
  expect(alive(hostPid), "repeat open did not reuse Host process");
  if (normalMode) {
    const repeatTargets = await waitFor(async () => {
      try {
        const response = await fetch(`http://127.0.0.1:${host.debugPort}/json/list`, {
          signal: AbortSignal.timeout(1_000),
        });
        const value = await response.json();
        const pages = value.filter(
          (item) => item.type === "page" && String(item.url) === String(shellTargets[0].url),
        );
        return pages.length === 1 ? pages : undefined;
      } catch {
        return undefined;
      }
    }, "reused Host app page");
    expect(
      repeatTargets[0].id === shellTargets[0].id && repeatTargets[0].url === shellTargets[0].url,
      "repeat Host invocation did not reuse the same app target",
    );
  }
  if (normalMode) {
    const initialHostPid = hostPid;
    await closeCdpTarget(host.debugPort, shellTargets[0]);
    await sleep(1_500);
    await waitFor(async () => {
      try {
        const response = await fetch(`http://127.0.0.1:${host.debugPort}/json/list`, {
          signal: AbortSignal.timeout(1_000),
        });
        const value = await response.json();
        return value.some((item) => item.id === shellTargets[0].id) ? undefined : true;
      } catch {
        return undefined;
      }
    }, "Shell target close");
    if (hostWarmTimeoutSeconds === 300) {
      expect(alive(initialHostPid), "warm Host exited before its configured timeout");
    } else {
      await waitGone(initialHostPid, "Host after window close");
      host = undefined;
    }
    if (hostWarmTimeoutSeconds === 300) {
      const reopen = spawn(
        hostExe,
        [`--user-data-dir=${hostUserData}`, "--open-app=com.kosmos.shell"],
        { env, stdio: "ignore", windowsHide: true },
      );
      const reopenExit = await new Promise((resolve, reject) => {
        reopen.once("error", reject);
        reopen.once("exit", resolve);
      });
      expect(reopenExit === 0, "Host reopen invocation did not hand off cleanly");
      expect(
        hostPid === initialHostPid && alive(hostPid),
        "Host reopen did not retain the warm Host process",
      );
    } else {
      host = await launchElectronDebug(hostExe, hostUserData, ["--open-app=com.kosmos.shell"]);
      hostPid = host.child.pid;
      expect(hostPid !== initialHostPid, "Host zero-timeout reopen did not start a fresh process");
    }
    hostWarmReuse = hostWarmTimeoutSeconds === 300 && hostPid === initialHostPid;
    const reopenedTargets = await waitFor(async () => {
      try {
        const response = await fetch(`http://127.0.0.1:${host.debugPort}/json/list`, {
          signal: AbortSignal.timeout(1_000),
        });
        const value = await response.json();
        const pages = value.filter((item) => {
          try {
            const url = new URL(String(item.url));
            return (
              item.type === "page" &&
              url.hostname === "127.0.0.1" &&
              Number(url.port) === lock.http_port &&
              /^\/v1\/apps\/assets\/[^/]+\/dist\/index\.html$/.test(url.pathname)
            );
          } catch {
            return false;
          }
        });
        return pages.length === 1 ? pages : undefined;
      } catch {
        return undefined;
      }
    }, "reopened Host app page");
    expect(reopenedTargets.length === 1, "reopened Host must expose exactly one Shell app page");
    expect(reopenedTargets[0].id !== shellTargets[0].id, "Host reopen reused a closed target");
    await closeCdpTarget(host.debugPort, reopenedTargets[0]);
    await stop(host.child, "Host");
    host = undefined;
  } else {
    await stop(host.child, "Host");
    await waitGone(hostPid, "Host");
    host = undefined;
  }

  for (const [operation, params] of [
    ["packages.set_enabled", { id: "com.kosmos.shell", version: shellVersion, enabled: false }],
    ["packages.uninstall", { id: "com.kosmos.shell", version: shellVersion }],
    ["packages.install", { id: "com.kosmos.shell", version: shellVersion }],
    ["packages.set_enabled", { id: "com.kosmos.shell", version: shellVersion, enabled: true }],
  ]) {
    const result = await rpc(operation, params);
    expect(
      result.ok,
      `Manager disable/uninstall/reinstall flow failed at ${operation}: ${result.error?.code ?? "unknown"}`,
    );
  }
  await stop(manager.child, "Manager");
  manager = undefined;
  const started = await rpc("dictation.start_recording");
  expect(
    started.ok && started.data?.state === "recording",
    "Runtime dictation did not enter recording",
  );
  const cancelled = await rpc("dictation.cancel");
  expect(cancelled.ok, "Runtime dictation cancellation failed");
  const idle = await waitFor(
    async () => {
      const state = await rpc("dictation.get_state");
      return state.ok && state.data?.state === "idle" ? state : undefined;
    },
    "dictation idle",
    10_000,
  );
  expect(idle.data.state === "idle", "Runtime dictation did not return to idle");
  if (dictationNativeNegative) {
    const missingModelPath = path.join(root, "missing-dictation-model.bin");
    const missingCommandPath = path.join(root, "missing-dictation-runtime.exe");
    const availableModelPath = path.join(root, "available-dictation-model.bin");
    const availableCommandPath = path.join(root, "available-dictation-runtime.exe");
    fs.writeFileSync(availableModelPath, "candidate smoke model placeholder");
    fs.writeFileSync(availableCommandPath, "candidate smoke command placeholder");
    const configured = await rpc("dictation.update_config", {
      provider: "local",
      providerEnabled: true,
      localEngine: "whisper.cpp",
      localModelId: "packaged-negative-missing-model",
      localModelPath: missingModelPath,
      localCommandPath: availableCommandPath,
      injectMode: "clipboard_only",
    });
    expect(
      configured.ok &&
        configured.data?.config?.provider === "local" &&
        configured.data?.config?.providerEnabled === false &&
        configured.data?.config?.localModel == null,
      "missing local Dictation assets were not cleared fail-closed",
    );
    const negativeStarted = await rpc("dictation.start_recording");
    expect(
      negativeStarted.ok && negativeStarted.data?.state === "recording",
      "native-negative Dictation recording did not start",
    );
    const negativeSubmitted = await rpc("dictation.submit_audio", {
      audioB64: "ZmFrZS1wYWNrYWdlZC1kaWN0YXRpb24tYXVkaW8=",
      durationSec: 1,
    });
    expect(
      negativeSubmitted.ok &&
        negativeSubmitted.data?.state === "error" &&
        String(negativeSubmitted.data?.error ?? "").includes("Локальная модель"),
      `missing local Dictation assets did not fail closed: ${JSON.stringify(negativeSubmitted)}`,
    );
    const negativeState = await rpc("dictation.get_state");
    expect(
      negativeState.ok &&
        negativeState.data?.state === "error" &&
        negativeState.data?.activeUuid === negativeSubmitted.data?.uuid &&
        String(negativeState.data?.lastError ?? "").includes("Локальная модель"),
      `native-negative Dictation error was not visible: ${JSON.stringify(negativeState)}`,
    );
    const pending = await rpc("dictation.list_pending");
    const pendingItems = pending.data?.items;
    expect(
      pending.ok &&
        Array.isArray(pendingItems) &&
        pendingItems.length === 1 &&
        pendingItems[0]?.uuid === negativeSubmitted.data?.uuid,
      `native-negative Dictation pending item was not visible: ${JSON.stringify(pending)}`,
    );
    const discarded = await rpc("dictation.discard", { uuid: negativeSubmitted.data.uuid });
    expect(discarded.ok && discarded.data?.discarded === true, "pending Dictation cleanup failed");
    const pendingAfterDiscard = await rpc("dictation.list_pending");
    const stateAfterDiscard = await rpc("dictation.get_state");
    expect(
      pendingAfterDiscard.ok &&
        Array.isArray(pendingAfterDiscard.data?.items) &&
        pendingAfterDiscard.data.items.length === 0 &&
        stateAfterDiscard.ok &&
        stateAfterDiscard.data?.state === "idle" &&
        stateAfterDiscard.data?.activeUuid == null,
      "pending Dictation cleanup did not return Runtime to idle",
    );
    const missingCommandConfigured = await rpc("dictation.update_config", {
      provider: "local",
      providerEnabled: true,
      localEngine: "whisper.cpp",
      localModelId: "packaged-negative-missing-command",
      localModelPath: availableModelPath,
      localCommandPath: missingCommandPath,
      injectMode: "clipboard_only",
    });
    expect(
      missingCommandConfigured.ok &&
        missingCommandConfigured.data?.config?.provider === "local" &&
        missingCommandConfigured.data?.config?.providerEnabled === false &&
        missingCommandConfigured.data?.config?.localModel == null,
      "missing local Dictation command was not cleared fail-closed",
    );
    const commandNegativeStarted = await rpc("dictation.start_recording");
    expect(
      commandNegativeStarted.ok && commandNegativeStarted.data?.state === "recording",
      "missing-command Dictation recording did not start",
    );
    const commandNegativeSubmitted = await rpc("dictation.submit_audio", {
      audioB64: "ZmFrZS1wYWNrYWdlZC1kaWN0YXRpb24tY29tbWFuZA==",
      durationSec: 1,
    });
    expect(
      commandNegativeSubmitted.ok &&
        commandNegativeSubmitted.data?.state === "error" &&
        String(commandNegativeSubmitted.data?.error ?? "").includes("Локальная модель"),
      `missing Dictation command did not fail closed: ${JSON.stringify(commandNegativeSubmitted)}`,
    );
    const commandDiscarded = await rpc("dictation.discard", {
      uuid: commandNegativeSubmitted.data.uuid,
    });
    expect(
      commandDiscarded.ok && commandDiscarded.data?.discarded === true,
      "missing-command Dictation cleanup failed",
    );
  }
  summary = {
    result: "pass",
    publicCatalog: "default",
    shellVersion,
    catalogSequence: trust.data.catalog.sequence,
    hostWarmTimeoutSeconds: normalMode ? hostWarmTimeoutSeconds : null,
    hostWarmReuse,
    dictationNativeNegative,
    before,
    ownedDuring: {
      runtimePid: engine.pid,
      managerPid,
      hostPid,
      windows,
      expectedAppPages: {
        manager: { type: "gpui-process", pid: managerPid },
        host: shellTargets.map(redactedTarget),
      },
    },
  };
} finally {
  const cleanupErrors = [];
  const attemptCleanup = async (label, action) => {
    try {
      await action();
    } catch (error) {
      cleanupErrors.push(`${label}: ${error instanceof Error ? error.message : String(error)}`);
    }
  };
  await attemptCleanup("Host", () => stop(host?.child, "Host"));
  await attemptCleanup("Manager", () => stop(manager?.child, "Manager"));
  await attemptCleanup("Runtime", () => terminate(engine, "Runtime"));
  await attemptCleanup("Runtime locks", () =>
    waitFor(
      () =>
        ["engine.lock.json", "kepler.lock.json", "engine-supervisor-state.json"].some((name) =>
          fs.existsSync(path.join(dataDir, name)),
        )
          ? undefined
          : true,
      "Runtime locks",
      10_000,
    ).catch(() => {
      const remaining = ["engine.lock.json", "kepler.lock.json", "engine-supervisor-state.json"]
        .filter((name) => fs.existsSync(path.join(dataDir, name)))
        .join(", ");
      throw new Error(`timed out waiting for removal: ${remaining}`);
    }),
  );
  const afterConsoles = consoleInventory();
  const extraVisibleConsoles = consoleDelta(before.consoles, afterConsoles).filter(
    (item) => item.visibleWindow,
  );
  try {
    cleanupRoot();
  } catch (error) {
    cleanupErrors.push(
      `data root cleanup: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const afterOwned = windowInventory([engine?.pid, managerPid, hostPid].filter(Boolean));
  const afterKosmos = kosmosProcessInventory();
  const newKosmosProcesses = kosmosProcessDelta(before.kosmosProcesses, afterKosmos);
  if (normalMode && extraVisibleConsoles.length > 0)
    cleanupErrors.push("normal smoke exposed a console window");
  if (normalMode && afterOwned.length > 0) cleanupErrors.push("normal smoke left owned processes");
  if (normalMode && newKosmosProcesses.length > 0)
    cleanupErrors.push("normal smoke left Kosmos processes");
  if (cleanupErrors.length > 0) {
    cleanupErrors.push(
      `after cleanup: owned=${afterOwned.length}, kosmos=${newKosmosProcesses.length}, root=${fs.existsSync(root) ? "present" : "empty"}`,
    );
  }
  if (summary)
    console.log(
      JSON.stringify({
        ...summary,
        normalMode,
        consoleDelta: {
          before: before.consoles,
          after: afterConsoles,
          extraVisible: extraVisibleConsoles,
        },
        afterCleanup: {
          processes: afterOwned,
          windows: afterOwned.filter((item) => item.visibleWindow),
          kosmosProcesses: newKosmosProcesses,
          dataRoot: fs.existsSync(root) ? "present" : "empty",
          errors: cleanupErrors,
        },
      }),
    );
  if (cleanupErrors.length > 0) {
    process.exitCode = 1;
    console.error(`cleanup incomplete: ${cleanupErrors.join("; ")}`);
  }
}
