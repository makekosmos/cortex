import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");
const taskRoot = path.resolve(__dirname, "..");
const label = process.argv[2] ?? "baseline";
const flags = JSON.parse(process.argv[3] ?? "{}");
const dataDir = path.join(taskRoot, "smoke", label);
const backendExe = path.join(repoRoot, "target", "debug", "kepler-backend.exe");
const lockPath = path.join(dataDir, "kepler.lock.json");
const resultPath = path.join(__dirname, `${label}-backend-bench.json`);
const typeperfPath = path.join(__dirname, `${label}-typeperf.csv`);
const processSamplerPath = path.join(__dirname, `${label}-process-sampler.csv`);

fs.rmSync(dataDir, { recursive: true, force: true });
fs.mkdirSync(dataDir, { recursive: true });

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForFile(file, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (fs.existsSync(file)) return;
    await sleep(100);
  }
  throw new Error(`Timed out waiting for ${file}`);
}

function percentile(values, p) {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.floor(((sorted.length - 1) * p) / 100);
  return sorted[index] ?? 0;
}

function stats(samples) {
  return {
    count: samples.length,
    p50_ms: percentile(samples, 50),
    p95_ms: percentile(samples, 95),
    max_ms: samples.length ? Math.max(...samples) : 0,
  };
}

async function connect(lock, clientId) {
  const ws = new WebSocket(`ws://127.0.0.1:${lock.ws_port}`);
  const pending = new Map();
  let events = 0;
  let payloadBytes = 0;
  ws.addEventListener("message", (event) => {
    const text = String(event.data);
    payloadBytes += Buffer.byteLength(text);
    const msg = JSON.parse(text);
    if (msg.event) {
      events += 1;
      return;
    }
    const id = msg.id;
    const item = pending.get(id);
    if (!item) return;
    pending.delete(id);
    item.resolve({ msg, bytes: Buffer.byteLength(text) });
  });
  await new Promise((resolve, reject) => {
    ws.addEventListener("open", resolve, { once: true });
    ws.addEventListener("error", reject, { once: true });
  });
  ws.send(
    JSON.stringify({
      kind: "hello",
      protocolVersion: `${lock.protocol_version.major}.${lock.protocol_version.minor}.${lock.protocol_version.patch}`,
      token: lock.auth_token,
      pid: process.pid,
      clientId,
    }),
  );
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("hello timeout")), 5_000);
    const onMessage = (event) => {
      const msg = JSON.parse(String(event.data));
      if (msg.kind === "hello_ok") {
        clearTimeout(timer);
        ws.removeEventListener("message", onMessage);
        resolve();
      } else if (msg.kind === "hello_error") {
        clearTimeout(timer);
        reject(new Error(msg.message));
      }
    };
    ws.addEventListener("message", onMessage);
  });
  let nextId = 1;
  return {
    ws,
    get events() {
      return events;
    },
    get payloadBytes() {
      return payloadBytes;
    },
    request(operation, params = {}) {
      const id = `${clientId}-${nextId++}`;
      const started = performance.now();
      ws.send(JSON.stringify({ _req_id: id, operation, ...params }));
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`${operation} timed out`));
        }, 30_000);
        pending.set(id, {
          resolve: (value) => {
            clearTimeout(timer);
            resolve({ ...value, ms: performance.now() - started });
          },
        });
      });
    },
    close() {
      ws.close();
    },
  };
}

async function measure(client, operation, params, count) {
  const latencies = [];
  const payloads = [];
  let last = null;
  for (let index = 0; index < count; index += 1) {
    const response = await client.request(operation, params);
    latencies.push(response.ms);
    payloads.push(response.bytes);
    last = response.msg;
  }
  return {
    latency: stats(latencies),
    payload_bytes: stats(payloads),
    last_count:
      last?.data?.apps?.length ??
      last?.data?.commands?.length ??
      last?.data?.results?.length ??
      null,
  };
}

async function waitForApps(client) {
  for (let index = 0; index < 80; index += 1) {
    const res = await client.request("app_index.list_all", { limit: 500 });
    if ((res.msg.data?.apps?.length ?? 0) > 0) return;
    await sleep(250);
  }
}

async function main() {
  const env = {
    ...process.env,
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
    RUST_BACKTRACE: "1",
    ...flags,
  };
  const backend = spawn(backendExe, [], {
    cwd: repoRoot,
    env,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  const backendLog = [];
  backend.stdout.on("data", (chunk) => backendLog.push(String(chunk)));
  backend.stderr.on("data", (chunk) => backendLog.push(String(chunk)));

  let typeperf = null;
  let typeperfExit = Promise.resolve({ code: null, signal: "not_started" });
  const typeperfLog = [];
  let processSampler = null;
  let processSamplerExit = Promise.resolve({ code: null, signal: "not_started" });
  const processSamplerLog = [];

  try {
    await waitForFile(lockPath, 20_000);
    const lock = JSON.parse(fs.readFileSync(lockPath, "utf8"));
    fs.rmSync(processSamplerPath, { force: true });
    const samplerCommand = [
      `$ErrorActionPreference = 'Continue'`,
      `for ($i = 0; $i -lt 120; $i++) {`,
      `  $p = Get-Process -Id ${lock.pid} -ErrorAction SilentlyContinue`,
      `  if ($p) {`,
      `    [pscustomobject]@{`,
      `      timestamp = (Get-Date).ToString('o')`,
      `      pid = $p.Id`,
      `      cpu_seconds = $p.CPU`,
      `      working_set = $p.WorkingSet64`,
      `      private_memory = $p.PrivateMemorySize64`,
      `      handles = $p.HandleCount`,
      `      threads = $p.Threads.Count`,
      `    } | Export-Csv -Path '${processSamplerPath.replaceAll("'", "''")}' -NoTypeInformation -Append`,
      `  }`,
      `  Start-Sleep -Seconds 1`,
      `}`,
    ].join("\n");
    processSampler = spawn("powershell.exe", ["-NoProfile", "-Command", samplerCommand], {
      windowsHide: true,
    });
    processSampler.stdout.on("data", (chunk) => processSamplerLog.push(String(chunk)));
    processSampler.stderr.on("data", (chunk) => processSamplerLog.push(String(chunk)));
    processSamplerExit = new Promise((resolve) => {
      processSampler.on("exit", (code, signal) => resolve({ code, signal }));
    });
    typeperf = spawn(
      "typeperf",
      [
        "\\Process(kepler-backend)\\% Processor Time",
        "\\Process(kepler-backend)\\Working Set",
        "\\Process(kepler-backend)\\IO Data Bytes/sec",
        "\\PhysicalDisk(_Total)\\Disk Bytes/sec",
        "-si",
        "1",
        "-sc",
        "120",
        "-f",
        "CSV",
        "-o",
        typeperfPath,
      ],
      { windowsHide: true },
    );
    typeperf.stdout.on("data", (chunk) => typeperfLog.push(String(chunk)));
    typeperf.stderr.on("data", (chunk) => typeperfLog.push(String(chunk)));
    typeperfExit = new Promise((resolve) => {
      typeperf.on("exit", (code, signal) => resolve({ code, signal }));
    });
    const client = await connect(lock, `${label}-measure`);
    await waitForApps(client);

    const beforeSnapshot = await client.request("diagnostics.snapshot");
    const appList = await measure(client, "app_index.list_all", { limit: 500 }, 30);
    const commands = await measure(client, "commands.list", {}, 50);
    const fileSearch = await measure(
      client,
      "file_index.search",
      { query: "kosmos", limit: 8 },
      20,
    );
    const afterSnapshot = await client.request("diagnostics.snapshot");

    const stormClient = await connect(lock, `${label}-storm`);
    let stormSent = 0;
    const stormStopAt = Date.now() + 60_000;
    const stormTimer = setInterval(() => {
      if (Date.now() >= stormStopAt) return;
      stormSent += 1;
      void stormClient
        .request("commands.invoke", {
          id: "bench.noop",
          params: { n: stormSent },
        })
        .catch(() => {});
    }, 60);
    const stormLatencies = [];
    while (Date.now() < stormStopAt) {
      const response = await client.request("commands.list");
      stormLatencies.push(response.ms);
      await sleep(250);
    }
    clearInterval(stormTimer);
    await sleep(500);
    const stormSnapshot = await client.request("diagnostics.snapshot");

    client.close();
    stormClient.close();

    const typeperfStatus = await typeperfExit;
    const processSamplerStatus = await processSamplerExit;
    const result = {
      label,
      flags,
      dataDir,
      typeperfPath,
      typeperfStatus,
      typeperfLog: typeperfLog.join("").split(/\r?\n/).filter(Boolean).slice(-40),
      processSamplerPath,
      processSamplerStatus,
      processSamplerLog: processSamplerLog.join("").split(/\r?\n/).filter(Boolean).slice(-40),
      backendPid: lock.pid,
      beforeSnapshot: beforeSnapshot.msg.data,
      afterSnapshot: afterSnapshot.msg.data,
      stormSnapshot: stormSnapshot.msg.data,
      app_index_list_all: appList,
      commands_list: commands,
      file_index_search: fileSearch,
      event_storm: {
        duration_s: 60,
        target_events_per_min: 1000,
        sent: stormSent,
        received_by_measure_client: client.events,
        commands_list_latency: stats(stormLatencies),
      },
      backendLogTail: backendLog.join("").split(/\r?\n/).slice(-80),
    };
    fs.writeFileSync(resultPath, JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result, null, 2));
  } finally {
    if (typeperf && !typeperf.killed) typeperf.kill();
    if (processSampler && !processSampler.killed) processSampler.kill();
    if (!backend.killed) backend.kill();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
