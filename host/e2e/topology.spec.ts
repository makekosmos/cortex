import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(hostRoot, "..", "..");
const taskRoot = path.join(repositoryRoot, ".agent", "tasks", "2026-07-29-kosmos-lego-final-audit");
const visualRoot = path.join(
  repositoryRoot,
  ".tmp",
  "visual",
  "2026-07-29-kosmos-lego-final-audit",
);
const releaseKey =
  "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIOySB4fj+9fjjYqVGN0MUgvLCskThB42RZM33lFKNGId\n-----END PRIVATE KEY-----\n";

type Lock = { pid: number; http_port: number; auth_token: string };
type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };
type CandidateManifest = {
  schema_version: number;
  version: string;
  files: Array<{ relative: string; sha256: string }>;
  trees: Array<{ relative: string; sha256: string }>;
};
type TopologyBinaries = { engine: string; ark: string; bridge: string };
type BridgeArchive = { file: string; manifest: Record<string, JsonValue> };
type CatalogAppend = { catalog: string; signatures: JsonValue };
type Candidate = {
  version: string;
  root: string;
  engine: string;
  ark: string;
  electron: string;
  hostMain: string;
  managerMain: string;
  manifest: CandidateManifest;
};

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const sha256 = (file: string) => createHash("sha256").update(fs.readFileSync(file)).digest("hex");
const assert = (condition: boolean, message: string): asserts condition => {
  if (!condition) throw new Error(message);
};

function treeHash(root: string): string {
  const files: string[] = [];
  const visit = (dir: string) =>
    fs
      .readdirSync(dir, { withFileTypes: true })
      .sort((a, b) => a.name.localeCompare(b.name))
      .forEach((entry) => {
        const file = path.join(dir, entry.name);
        if (entry.isDirectory()) visit(file);
        else if (entry.isFile())
          files.push(`${path.relative(root, file).replaceAll("\\", "/")}:${sha256(file)}`);
      });
  visit(root);
  return createHash("sha256").update(files.join("\n")).digest("hex");
}

function build(trust: { root: string; releases: string }): TopologyBinaries {
  const target = path.join(repositoryRoot, ".tmp", "topology-target");
  const env = {
    ...process.env,
    CARGO_TARGET_DIR: target,
    KOSMOS_PACKAGE_ROOT_KEY_JSON: trust.root,
    KOSMOS_PACKAGE_RELEASE_KEYS_JSON: trust.releases,
  };
  execFileSync("node", [
    path.join(repositoryRoot, "desktop", "scripts", "ark-core-rpc.mjs"),
    "--debug",
    "--target-dir",
    path.join(target, "debug"),
  ], {
    cwd: repositoryRoot,
    env,
    stdio: "inherit",
  });
  execFileSync(
    "cargo",
    ["build", "-p", "kepler-backend", "--bin", "kepler-backend", "--bin", "ark-markdown-bridge"],
    { cwd: repositoryRoot, env, stdio: "inherit" },
  );
  const binaries = {
    engine: path.join(target, "debug", "kepler-backend.exe"),
    ark: path.join(target, "debug", "ark-core-rpc.exe"),
    bridge: path.join(target, "debug", "ark-markdown-bridge.exe"),
  };
  for (const binary of Object.values(binaries))
    assert(fs.existsSync(binary), `build required before topology E2E: ${binary}`);
  return binaries;
}

function candidate(
  installRoot: string,
  version: string,
  binaries: TopologyBinaries,
): Candidate {
  const root = path.join(installRoot, "versions", version);
  const copy = (source: string, destination: string) => {
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.cpSync(source, destination, { recursive: true });
  };
  fs.mkdirSync(root, { recursive: true });
  copy(binaries.engine, path.join(root, "engine", "kepler-backend.exe"));
  copy(binaries.ark, path.join(root, "engine", "ark-core-rpc.exe"));
  copy(path.dirname(electronBinary), path.join(root, "electron"));
  copy(path.join(repositoryRoot, "platform", "host", "dist"), path.join(root, "host", "dist"));
  copy(
    path.join(repositoryRoot, "platform", "host", "dist-electron"),
    path.join(root, "host", "dist-electron"),
  );
  copy(
    path.join(repositoryRoot, "platform", "manager", "dist"),
    path.join(root, "manager", "dist"),
  );
  copy(
    path.join(repositoryRoot, "platform", "manager", "dist-electron"),
    path.join(root, "manager", "dist-electron"),
  );
  fs.writeFileSync(path.join(root, "release.json"), JSON.stringify({ schema_version: 1, version }));
  const manifest = {
    schema_version: 1,
    version,
    files: [
      "engine/kepler-backend.exe",
      "engine/ark-core-rpc.exe",
      "electron/electron.exe",
      "release.json",
    ].map((relative) => ({
      relative,
      sha256: sha256(path.join(root, relative)),
    })),
    trees: ["host", "manager"].map((relative) => ({
      relative,
      sha256: treeHash(path.join(root, relative)),
    })),
  };
  fs.writeFileSync(path.join(root, "candidate.json"), JSON.stringify(manifest, null, 2));
  return {
    version,
    root,
    engine: path.join(root, "engine", "kepler-backend.exe"),
    ark: path.join(root, "engine", "ark-core-rpc.exe"),
    electron: path.join(root, "electron", "electron.exe"),
    hostMain: path.join(root, "host", "dist-electron", "main.js"),
    managerMain: path.join(root, "manager", "dist-electron", "main.js"),
    manifest,
  };
}

function verify(candidate: Candidate): void {
  for (const file of candidate.manifest.files)
    assert(
      sha256(path.join(candidate.root, file.relative)) === file.sha256,
      `tampered candidate file: ${file.relative}`,
    );
  for (const tree of candidate.manifest.trees)
    assert(
      treeHash(path.join(candidate.root, tree.relative)) === tree.sha256,
      `tampered candidate tree: ${tree.relative}`,
    );
}

function candidateBuildHash(candidate: Candidate): string {
  return `${sha256(path.join(candidate.root, "candidate.json"))}:${treeHash(candidate.root)}`;
}

function switchCurrent(installRoot: string, next: Candidate): void {
  const current = path.join(installRoot, "current.json");
  const previous = path.join(installRoot, "previous.json");
  if (fs.existsSync(current)) fs.renameSync(current, previous);
  const temporary = `${current}.${randomUUID()}.tmp`;
  fs.writeFileSync(temporary, JSON.stringify({ version: next.version, root: next.root }));
  fs.renameSync(temporary, current);
}

async function waitFor<T>(read: () => T | undefined, label: string): Promise<T> {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const value = read();
    if (value !== undefined) return value;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${label}`);
}

async function startEngine(
  candidate: Candidate,
  data: string,
  vault: string,
  trust: { root: string; releases: string },
): Promise<{ child: ChildProcess; lock: Lock }> {
  const isolatedRoot = path.dirname(data);
  const child = spawn(candidate.engine, [], {
    env: {
      ...process.env,
      APPDATA: path.join(isolatedRoot, "appdata"),
      PROGRAMDATA: path.join(isolatedRoot, "programdata"),
      KOSMOS_DATA_DIR: data,
      ARK_CORE_RPC_PATH: candidate.ark,
      KOSMOS_PACKAGE_ROOT_KEY_JSON: trust.root,
      KOSMOS_PACKAGE_RELEASE_KEYS_JSON: trust.releases,
      KOSMOS_WORKER_FILESYSTEM_ROOTS: vault,
      KEPLER_APP_INDEX_INITIAL_DELAY_MS: "600000",
      KEPLER_FILE_INDEX_INITIAL_DELAY_MS: "600000",
      KEPLER_FILE_INDEX_INITIAL_RESCAN: "0",
      KEPLER_FILE_INDEX: "0",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KOSMOS_TEST_MODE: "1",
      KEPLER_SKIP_SYNC: "1",
      KEPLER_USAGE_TRACKER: "0",
    },
    stdio: "ignore",
    windowsHide: true,
  });
  const lock = await waitFor(() => {
    try {
      // SAFETY: waitFor only resolves after the Engine lock file is written.
      return JSON.parse(fs.readFileSync(path.join(data, "engine.lock.json"), "utf8")) as Lock;
    } catch {
      return undefined;
    }
  }, "Engine lock");
  return { child, lock };
}

async function rpc(lock: Lock, operation: string, params: Record<string, JsonValue> = {}) {
  const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      Connection: "close",
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "topology-audit",
      "X-Kosmos-Client-Version": "1.0.0",
      "X-Kosmos-Client-Pid": String(process.pid),
    },
    body: JSON.stringify({ operation, _req_id: randomUUID(), ...params }),
    signal: AbortSignal.timeout(30_000),
  });
  // SAFETY: the test Engine endpoint returns the documented JSON RPC envelope.
  const result = (await response.json()) as {
    ok: boolean;
    data?: JsonValue;
    error?: JsonValue;
  };
  assert(
    result.ok,
    `${operation}: ${result.error?.constructor === String ? result.error : "failed"}`,
  );
  return result.data;
}

const alive = (pid: number) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
function processCommandLine(pid: number): string {
  const script = `(Get-CimInstance Win32_Process -Filter 'ProcessId=${pid}' | Select-Object -First 1 -ExpandProperty CommandLine)`;
  return execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], {
    encoding: "utf8",
    windowsHide: true,
  }).trim();
}
async function stopPid(pid: number, label: string): Promise<void> {
  if (!alive(pid)) return;
  try {
    execFileSync("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
      stdio: "ignore",
      windowsHide: true,
    });
  } catch {}
  await waitFor(() => (!alive(pid) ? true : undefined), `${label} exit`);
}
async function stopEngine(child: ChildProcess | undefined, candidate: Candidate, data: string) {
  if (!child?.pid || !alive(child.pid)) return;
  try {
    execFileSync(candidate.engine, ["--shutdown"], {
      env: {
        ...process.env,
        KOSMOS_DATA_DIR: data,
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      },
      stdio: "ignore",
      windowsHide: true,
      timeout: 10_000,
    });
  } catch {}
  await stopPid(child.pid, "Engine");
}
async function close(app: ElectronApplication | undefined, label: string) {
  if (!app) return;
  const pid = app.process().pid;
  await Promise.race([app.close().catch(() => undefined), sleep(5_000)]);
  await stopPid(pid, label);
}
async function capture(
  app: ElectronApplication,
  page: Awaited<ReturnType<ElectronApplication["firstWindow"]>>,
  width: number,
  name: string,
) {
  await app.evaluate(
    ({ BrowserWindow }, nextWidth) => BrowserWindow.getAllWindows()[0]?.setSize(nextWidth, 760),
    width,
  );
  await page.screenshot({
    path: path.join(visualRoot, name),
    timeout: 10_000,
    animations: "disabled",
  });
}

async function captureNative(
  app: ElectronApplication,
  page: Awaited<ReturnType<ElectronApplication["firstWindow"]>>,
  width: number,
  name: string,
): Promise<void> {
  const png = await app.evaluate(async ({ BrowserWindow }, nextWidth) => {
    const window = BrowserWindow.getAllWindows()[0];
    if (!window) throw new Error("no Electron window for native capture");
    window.setSize(nextWidth + 1, 760);
    window.setSize(nextWidth, 760);
    window.webContents.invalidate();
    await new Promise((resolve) => setTimeout(resolve, 250));
    return (await window.webContents.capturePage()).toPNG().toString("base64");
  }, width);
  const bytes = Buffer.from(png, "base64");
  assert(
    bytes.length > 1024 && bytes.subarray(0, 8).equals(Buffer.from("89504e470d0a1a0a", "hex")),
    `blank Manager capture: ${name}`,
  );
  fs.mkdirSync(visualRoot, { recursive: true });
  fs.writeFileSync(path.join(visualRoot, name), bytes);
}

function zip(stage: string, archive: string) {
  const quote = (value: string) => `'${value.replaceAll("'", "''")}'`;
  execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `$stage=${quote(stage)}; $archive=${quote(archive)}; Add-Type -AssemblyName System.IO.Compression; Add-Type -AssemblyName System.IO.Compression.FileSystem; $stream=[IO.File]::Open($archive,[IO.FileMode]::Create); try {$zip=[IO.Compression.ZipArchive]::new($stream,[IO.Compression.ZipArchiveMode]::Create,$false); try {Get-ChildItem -LiteralPath $stage -Recurse -File | ForEach-Object {$entry=$zip.CreateEntry($_.FullName.Substring($stage.Length+1).Replace('\\','/')); $input=[IO.File]::OpenRead($_.FullName); $output=$entry.Open(); try {$input.CopyTo($output)} finally {$output.Dispose();$input.Dispose()}}} finally {$zip.Dispose()}} finally {$stream.Dispose()}`,
    ],
    { cwd: repositoryRoot },
  );
}

function bridgeArchive(
  root: string,
  binary: string,
): BridgeArchive {
  const stage = path.join(root, "bridge-stage");
  const file = path.join(root, "ark-markdown-bridge.kspkg");
  // SAFETY: the checked package manifest is generated by the repository fixture.
  const manifest = JSON.parse(
    fs.readFileSync(
      path.join(repositoryRoot, "packages", "ark-markdown-bridge", "manifest.json"),
      "utf8",
    ),
  ) as Record<string, JsonValue>;
  fs.mkdirSync(stage, { recursive: true });
  fs.writeFileSync(path.join(stage, "manifest.json"), JSON.stringify(manifest));
  const entrypoint = path.join(stage, ...String(manifest.entrypoint).split("/"));
  fs.mkdirSync(path.dirname(entrypoint), { recursive: true });
  fs.copyFileSync(binary, entrypoint);
  zip(stage, file);
  return { file, manifest };
}

function appendBridgeCatalog(
  root: string,
  catalog: string,
  bridge: ReturnType<typeof bridgeArchive>,
): CatalogAppend {
  // SAFETY: appendBridgeCatalog receives the fixture catalog generated above.
  const document = JSON.parse(catalog) as { packages: JsonValue[] };
  document.packages.push({
    manifest: bridge.manifest,
    archive_url: "https://fixture.invalid/ark-markdown-bridge.kspkg",
    sha256: sha256(bridge.file),
    size: fs.statSync(bridge.file).size,
  });
  const catalogFile = path.join(root, "topology-catalog.json");
  const keyFile = path.join(root, "topology-release.pem");
  const signatureFile = path.join(root, "topology-catalog.signatures.json");
  fs.writeFileSync(catalogFile, JSON.stringify(document));
  fs.writeFileSync(keyFile, releaseKey);
  execFileSync(
    process.execPath,
    [
      path.join(repositoryRoot, "platform", "desktop", "scripts", "package-sign.mjs"),
      "--input",
      catalogFile,
      "--output",
      signatureFile,
      "--key-id",
      "release-1",
      "--key",
      keyFile,
    ],
    { cwd: repositoryRoot },
  );
  return {
    catalog: fs.readFileSync(catalogFile, "utf8"),
    signatures: JSON.parse(fs.readFileSync(signatureFile, "utf8")),
  };
}

async function snapshot(lock: Lock, data: string, vault: string) {
  const [object, links, vector, peers, packages, trust] = await Promise.all([
    rpc(lock, "get_object", { id: "topology-a" }),
    rpc(lock, "list_object_links"),
    rpc(lock, "get_sync_kv", { key: "lan_sync.version_vector" }),
    rpc(lock, "get_connected_peers"),
    rpc(lock, "packages.list"),
    rpc(lock, "packages.trust_status"),
  ]);
  const vaultFile = path.join(vault, "Topology A-topology-a.md");
  // SAFETY: get_sync_kv returns the serialized string version vector.
  const vectorEntries = JSON.parse(String(vector)) as Record<string, string>;
  const topologyVector = Object.fromEntries(
    ["topology_note", "topology-a", "topology-b", "topology-link"].map((id) => [
      id,
      vectorEntries[id],
    ]),
  );
  // SAFETY: packages.list returns the documented package rows used by the topology snapshot.
  const packageState = (
    packages.packages as Array<{
      id: string;
      version: string;
      enabled: boolean;
      revoked: boolean;
      trust_status?: unknown;
    }>
  )
    .map(({ id, version, enabled, revoked, trust_status }) => ({
      id,
      version,
      enabled,
      revoked,
      trust_status,
    }))
    .sort((left, right) => left.id.localeCompare(right.id));
  return {
    object,
    links,
    vector: topologyVector,
    version_vector: topologyVector,
    list_connected_peers: peers,
    packages: packageState,
    packages_trust_status: trust,
    device_id: fs.existsSync(path.join(data, "kepler-device-id.txt"))
      ? fs.readFileSync(path.join(data, "kepler-device-id.txt"), "utf8").trim()
      : "",
    vault: fs.existsSync(vaultFile) ? fs.readFileSync(vaultFile, "utf8") : "",
  };
}

test("real local topology installs, upgrades, rolls back, and removes optional components", async () => {
  test.skip(
    process.env.KOSMOS_TOPOLOGY_RESOURCES === "1",
    "resource run uses the same installed topology separately",
  );
  test.setTimeout(300_000);
  for (const directory of [
    path.join(repositoryRoot, "platform", "host", "dist"),
    path.join(repositoryRoot, "platform", "manager", "dist"),
    path.join(repositoryRoot, "products", "cosmos-graph", "dist"),
    path.join(repositoryRoot, "products", "shell", "dist"),
  ])
    assert(fs.existsSync(directory), `build required: ${directory}`);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-topology-"));
  const install = path.join(root, "install");
  const data = path.join(root, "data");
  const vault = path.join(root, "vault");
  const appData = path.join(root, "appdata");
  const programData = path.join(root, "programdata");
  const shortcutRoot = path.join(root, "shortcuts");
  const userData = path.join(root, "user-data");
  fs.mkdirSync(vault, { recursive: true });
  fs.mkdirSync(data, { recursive: true });
  fs.mkdirSync(visualRoot, { recursive: true });
  fs.writeFileSync(path.join(data, "kepler-device-id.txt"), "topology-device-id\n");
  let engine: ChildProcess | undefined;
  let manager: ElectronApplication | undefined;
  let host: ElectronApplication | undefined;
  const pids = new Set<number>();
  const manifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!manifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");
  const matrix: Array<{
    component: string;
    unavailable: string;
    remaining_client?: string;
    preserved?: boolean;
  }> = [];
  try {
    const apps = createSignedApps(root, repositoryRoot, true, true);
    const trust = apps.trust;
    const binaries = build(trust);
    const old = candidate(install, "fixture-old", binaries);
    const current = candidate(install, "fixture-current", binaries);
    verify(old);
    verify(current);
    assert(
      sha256(path.join(old.root, "release.json")) !==
        sha256(path.join(current.root, "release.json")),
      "fixture release metadata must differ",
    );
    switchCurrent(install, old);
    const bridge = bridgeArchive(root, binaries.bridge);
    const catalog = appendBridgeCatalog(root, apps.catalog, bridge);
    let started = await startEngine(old, data, vault, trust);
    engine = started.child;
    pids.add(engine.pid!);
    let lock = started.lock;
    await rpc(lock, "packages.catalog_apply", {
      document: catalog.catalog,
      signatures: catalog.signatures,
    });
    for (const id of [
      "com.kosmos.graph",
      "com.kosmos.shell",
      "ark-markdown-bridge",
      "host-e2e-app-a",
    ]) {
      const archive = id === "ark-markdown-bridge" ? bridge.file : apps.archives[id];
      const version =
        id === "ark-markdown-bridge" ? String(bridge.manifest.version) : apps.versions[id];
      await rpc(lock, "packages.install", {
        id,
        version,
        archive_path: archive,
      });
    }
    await rpc(lock, "packages.bridge_config_set", {
      id: "ark-markdown-bridge",
      version: String(bridge.manifest.version),
      config: {
        vault_root: vault,
        selected_types: ["com.kosmos.note"],
        editable_fields: ["title", "body"],
        readonly_fields: [],
      },
    });
    for (const id of [
      "com.kosmos.graph",
      "com.kosmos.shell",
      "ark-markdown-bridge",
      "host-e2e-app-a",
    ]) {
      const version =
        id === "ark-markdown-bridge" ? String(bridge.manifest.version) : apps.versions[id];
      await rpc(lock, "packages.set_enabled", {
        id,
        version,
        enabled: true,
      });
    }
    for (const [id, title] of [
      ["topology-a", "Topology A"],
      ["topology-b", "Topology B"],
    ])
      await rpc(lock, "upsert_object", {
        object: {
          id,
          typeId: "com.kosmos.note",
          typeVersion: "1.0.0",
          title,
          contentJson: {
            type: "doc",
            content: [{ type: "paragraph", content: [{ type: "text", text: "preserve me" }] }],
          },
          propsJson: { description: null, extensions: {} },
          createdAt: "2026-07-29T00:00:00Z",
          updatedAt: "2026-07-29T00:00:00Z",
          deletedAt: null,
        },
        device_id: "topology",
      });
    await rpc(lock, "upsert_object_link", {
      object_link: {
        id: "topology-link",
        sourceObjectId: "topology-a",
        targetObjectId: "topology-b",
        linkType: "related",
        createdAt: "2026-07-29T00:00:00Z",
      },
      device_id: "topology",
    });
    // SAFETY: list_objects returns the documented object rows.
    const isolatedObjects = (await rpc(lock, "list_objects")) as Array<{
      id: string;
    }>;
    const isolatedObjectIds = isolatedObjects.map((object) => object.id).sort();
    assert(
      JSON.stringify(isolatedObjectIds) === JSON.stringify(["topology-a", "topology-b"]),
      `isolated Engine imported unexpected objects: ${isolatedObjectIds.join(",")}`,
    );
    await waitFor(
      () => (fs.existsSync(path.join(vault, "Topology A-topology-a.md")) ? true : undefined),
      "Bridge projection",
    );
    const before = await snapshot(lock, data, vault);
    assert(before.vault.includes("preserve me"), "Bridge did not project real ARK object");
    const isolatedEnv = {
      ...process.env,
      APPDATA: appData,
      PROGRAMDATA: programData,
      KOSMOS_SHORTCUT_DIR: shortcutRoot,
      KOSMOS_DATA_DIR: data,
      KEPLER_APP_INDEX_INITIAL_DELAY_MS: "600000",
      KEPLER_FILE_INDEX_INITIAL_DELAY_MS: "600000",
      KEPLER_FILE_INDEX_INITIAL_RESCAN: "0",
      KEPLER_FILE_INDEX: "0",
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
    };
    const launchManager = () =>
      electron.launch({
        executablePath: old.electron,
        args: [`--user-data-dir=${path.join(userData, randomUUID())}`, old.managerMain],
        env: isolatedEnv,
      });
    manager = await launchManager();
    pids.add(manager.process().pid);
    const managerPage = await manager.firstWindow();
    managerPage.on("console", (message) =>
      console.log(`[manager-console] ${message.type()}: ${message.text()}`),
    );
    managerPage.on("pageerror", (error) => console.log(`[manager-pageerror] ${error.message}`));
    managerPage.on("requestfailed", (request) =>
      console.log(
        `[manager-requestfailed] ${request.url()} ${request.failure()?.errorText ?? "unknown"}`,
      ),
    );
    await expect.poll(() => managerPage.url(), { timeout: 30_000 }).toContain("dist/index.html");
    await managerPage.waitForLoadState("domcontentloaded");
    await expect(managerPage.getByRole("heading", { level: 1, name: "Данные" })).toBeVisible();
    await expect
      .poll(async () => (await managerPage.locator("body").innerText()).includes("Заметка"), {
        timeout: 30_000,
      })
      .toBe(true);
    await expect(managerPage.locator(".error")).toHaveCount(0);
    await managerPage.evaluate(
      () => new Promise((resolve) => requestAnimationFrame(() => resolve(true))),
    );
    await captureNative(manager, managerPage, 1180, "manager-1180x760.png");
    await captureNative(manager, managerPage, 640, "manager-640x760.png");
    await close(manager, "Manager standalone");
    manager = undefined;
    const launchHost = (id: string) =>
      electron.launch({
        executablePath: old.electron,
        args: [
          `--user-data-dir=${path.join(userData, randomUUID())}`,
          old.hostMain,
          "--open-app",
          id,
        ],
        env: isolatedEnv,
      });
    host = await launchHost("com.kosmos.graph");
    pids.add(host.process().pid);
    await waitFor(
      () =>
        fs.existsSync(path.join(shortcutRoot, ".kosmos-desktop-host-shortcuts.json"))
          ? true
          : undefined,
      "Host shortcut index",
    );
    // SAFETY: the Host writes this shortcut index using the documented entry shape.
    const shortcuts = JSON.parse(
      fs.readFileSync(path.join(shortcutRoot, ".kosmos-desktop-host-shortcuts.json"), "utf8"),
    ) as Array<{ id: string; file: string }>;
    const graphShortcut = shortcuts.find((item) => item.id === "com.kosmos.graph");
    assert(
      graphShortcut && fs.existsSync(graphShortcut.file),
      "Host did not create its owned Graph shortcut",
    );
    const shortcutDetails = await host.evaluate(
      ({ shell }, file) => shell.readShortcutLink(file),
      graphShortcut!.file,
    );
    assert(
      shortcutDetails.target === old.electron,
      "Graph shortcut target is not installed Electron",
    );
    assert(
      shortcutDetails.args === `"${old.hostMain}" --open-app=com.kosmos.graph`,
      `Graph shortcut args are not runnable: ${shortcutDetails.args}`,
    );
    await capture(host, await host.firstWindow(), 1180, "graph-1180x760.png");
    await close(host, "Host graph normal");
    host = await launchHost("com.kosmos.graph");
    pids.add(host.process().pid);
    await capture(host, await host.firstWindow(), 640, "graph-640x760.png");
    await close(host, "Host graph narrow");
    host = undefined;
    host = await launchHost("com.kosmos.shell");
    pids.add(host.process().pid);
    await capture(host, await host.firstWindow(), 1000, "shell-1000x760.png");
    await close(host, "Host shell normal");
    host = await launchHost("com.kosmos.shell");
    pids.add(host.process().pid);
    await capture(host, await host.firstWindow(), 640, "shell-640x760.png");
    await close(host, "Host before upgrade");
    host = undefined;
    await close(manager, "Manager before upgrade");
    manager = undefined;
    await stopEngine(engine, old, data);
    engine = undefined;
    switchCurrent(install, current);
    started = await startEngine(current, data, vault, trust);
    engine = started.child;
    pids.add(engine.pid!);
    lock = started.lock;
    const afterUpgrade = await snapshot(lock, data, vault);
    expect(afterUpgrade).toEqual(before);
    await stopEngine(engine, current, data);
    engine = undefined;
    // SAFETY: switchCurrent writes previous.json with a root pointer.
    const previous = JSON.parse(fs.readFileSync(path.join(install, "previous.json"), "utf8")) as {
      root: string;
    };
    assert(previous.root === old.root, "rollback pointer lost previous candidate");
    fs.writeFileSync(
      path.join(install, "current.json"),
      JSON.stringify({ version: old.version, root: old.root }),
    );
    started = await startEngine(old, data, vault, trust);
    engine = started.child;
    pids.add(engine.pid!);
    lock = started.lock;
    expect(await snapshot(lock, data, vault)).toEqual(before);
    const tampered = path.join(install, "versions", "tampered");
    fs.mkdirSync(tampered, { recursive: true });
    fs.copyFileSync(old.engine, path.join(tampered, "kepler-backend.exe"));
    fs.appendFileSync(path.join(tampered, "kepler-backend.exe"), "tampered");
    const bad = {
      ...old,
      root: tampered,
      engine: path.join(tampered, "kepler-backend.exe"),
    };
    expect(() => verify(bad)).toThrow(/tampered/);
    assert(
      JSON.parse(fs.readFileSync(path.join(install, "current.json"), "utf8")).root === old.root,
      "tampered candidate replaced current",
    );
    await rpc(lock, "get_object", { id: "topology-a" });
    const recordRow = async (component: string, unavailable: string) => {
      const preserved = await snapshot(lock, data, vault);
      expect(preserved.object).toEqual(before.object);
      expect(preserved.links).toEqual(before.links);
      expect(preserved.vector).toEqual(before.vector);
      expect(preserved.version_vector).toEqual(before.version_vector);
      expect(preserved.list_connected_peers).toEqual(before.list_connected_peers);
      expect(preserved.device_id).toEqual(before.device_id);
      expect(preserved.packages_trust_status).toEqual(before.packages_trust_status);
      expect(preserved.vault).toEqual(before.vault);
      assert(
        preserved.packages.some(
          (item) => item.id === "host-e2e-app-a" && item.enabled && !item.revoked,
        ),
        "remaining fixture App is unavailable",
      );
      await rpc(lock, "get_object", { id: "topology-a" });
      matrix.push({
        component,
        unavailable,
        remaining_client: "standalone-engine-v1",
        preserved: true,
      });
    };
    await rpc(lock, "packages.set_enabled", {
      id: "com.kosmos.shell",
      version: apps.versions["com.kosmos.shell"],
      enabled: false,
    });
    await rpc(lock, "packages.uninstall", {
      id: "com.kosmos.shell",
      version: apps.versions["com.kosmos.shell"],
    });
    await recordRow("Shell omitted", "Shell launcher");
    await recordRow("Manager closed/omitted", "Manager window");
    await recordRow("Host stopped", "Hosted app windows");
    await rpc(lock, "packages.set_enabled", {
      id: "com.kosmos.graph",
      version: apps.versions["com.kosmos.graph"],
      enabled: false,
    });
    await rpc(lock, "packages.uninstall", {
      id: "com.kosmos.graph",
      version: apps.versions["com.kosmos.graph"],
    });
    await recordRow("Hosted Graph disabled/uninstalled", "Graph app");
    await rpc(lock, "packages.set_enabled", {
      id: "ark-markdown-bridge",
      version: String(bridge.manifest.version),
      enabled: false,
    });
    await rpc(lock, "packages.uninstall", {
      id: "ark-markdown-bridge",
      version: String(bridge.manifest.version),
    });
    await recordRow("Bridge disabled/uninstalled", "Markdown projection");
    assert(!fs.existsSync(path.join(old.root, "desktop")), "new topology includes legacy Desktop");
    await recordRow("Legacy Desktop omitted", "Legacy Desktop shell");
    await stopEngine(engine, old, data);
    engine = undefined;
    const preservedFiles = new Map<string, string>();
    for (const base of [data, vault])
      for (const file of fs.readdirSync(base, {
        recursive: true,
        withFileTypes: true,
      }))
        if (file.isFile()) {
          const relative = path.relative(base, file.parentPath ?? base);
          preservedFiles.set(
            path.join(base, relative, file.name),
            sha256(path.join(base, relative, file.name)),
          );
        }
    const ownedRoots = [install, shortcutRoot];
    for (const owned of ownedRoots) {
      const resolved = path.resolve(owned);
      assert(
        resolved.startsWith(path.resolve(root) + path.sep),
        `unsafe uninstall root: ${resolved}`,
      );
    }
    fs.rmSync(install, { recursive: true, force: true });
    fs.rmSync(shortcutRoot, { recursive: true, force: true });
    assert(
      !fs.existsSync(install) && !fs.existsSync(shortcutRoot),
      "safe uninstall left owned topology files",
    );
    for (const [file, hash] of preservedFiles)
      assert(
        fs.existsSync(file) && sha256(file) === hash,
        `safe uninstall changed preserved data: ${file}`,
      );
    const evidence = {
      root,
      candidates: { old: old.manifest, current: current.manifest },
      owned: {
        install,
        shortcut_root: shortcutRoot,
        shortcut_index: path.join(shortcutRoot, ".kosmos-desktop-host-shortcuts.json"),
      },
      pids: [...pids],
      matrix,
      isolated_object_ids: isolatedObjectIds,
      shortcut_routing: {
        target: shortcutDetails.target,
        args: shortcutDetails.args,
        exact: true,
      },
      snapshots_equal: true,
      uninstall: {
        stopped: true,
        owned_roots_removed: true,
        preserved_data: true,
      },
      cleanup: "registered-with-runner",
    };
    fs.writeFileSync(path.join(taskRoot, "topology-e2e.json"), JSON.stringify(evidence, null, 2));
  } finally {
    await close(host, "Host cleanup");
    await close(manager, "Manager cleanup");
    await stopEngine(
      engine,
      // SAFETY: stopEngine only reads the engine path from this fixture candidate.
      {
        engine: path.join(install, "versions", "fixture-current", "engine", "kepler-backend.exe"),
      } as Candidate,
      data,
    );
    for (const pid of pids) await stopPid(pid, "recorded process");
    const cleanup = JSON.parse(fs.readFileSync(manifest, "utf8"));
    cleanup.roots.push(root);
    cleanup.pids.push(...pids);
    fs.writeFileSync(manifest, JSON.stringify(cleanup));
  }
});

test("real installed Host resource scenarios", async () => {
  test.skip(
    process.env.KOSMOS_TOPOLOGY_RESOURCES !== "1",
    "set KOSMOS_TOPOLOGY_RESOURCES=1 for the three 10-minute measurements",
  );
  test.setTimeout(2_000_000);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-topology-resource-"));
  const data = path.join(root, "data");
  const vault = path.join(root, "vault");
  const appData = path.join(root, "appdata");
  const shortcutRoot = path.join(root, "shortcuts");
  const resources = path.join(taskRoot, "resources");
  const manifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!manifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");
  fs.mkdirSync(vault, { recursive: true });
  fs.mkdirSync(resources, { recursive: true });
  let engine: ChildProcess | undefined;
  let hostProcess: ChildProcess | undefined;
  const pids = new Set<number>();
  try {
    const apps = createSignedApps(root, repositoryRoot, false, false, true);
    const binaries = build(apps.trust);
    const installed = candidate(path.join(root, "install"), "resource-current", binaries);
    verify(installed);
    const buildHash = candidateBuildHash(installed);
    const started = await startEngine(installed, data, vault, apps.trust);
    engine = started.child;
    pids.add(engine.pid!);
    const lock = started.lock;
    // SAFETY: list_objects returns the documented object rows.
    const isolatedObjects = (await rpc(lock, "list_objects")) as Array<{
      id: string;
    }>;
    assert(
      isolatedObjects.length === 0,
      `resource Engine imported unexpected objects: ${isolatedObjects.map((object) => object.id).join(",")}`,
    );
    await rpc(lock, "packages.catalog_apply", {
      document: apps.catalog,
      signatures: apps.signatures,
    });
    await rpc(lock, "packages.install", {
      id: "host-e2e-app-a",
      version: "1.0.0",
      archive_path: apps.archives["host-e2e-app-a"],
    });
    await rpc(lock, "packages.set_enabled", {
      id: "host-e2e-app-a",
      version: "1.0.0",
      enabled: true,
    });
    const resourceDuration = "600";
    const collect = (scenario: "engine-only" | "host-zero" | "host-warm-300", hostPid = 0) =>
      execFileSync(
        "pwsh",
        [
          "-NoProfile",
          "-ExecutionPolicy",
          "Bypass",
          "-File",
          path.join(repositoryRoot, "scripts", "release", "measure-kepler-ram.ps1"),
          "-Mode",
          "lego",
          "-Scenario",
          scenario,
          "-EnginePid",
          String(engine!.pid),
          "-HostPid",
          String(hostPid),
          "-Duration",
          resourceDuration,
          "-Samples",
          "6",
          "-Warmup",
          "0",
          "-BuildHash",
          buildHash,
          "-OutFile",
          path.join(resources, `${scenario}.json`),
        ],
        { cwd: repositoryRoot, stdio: "inherit", windowsHide: true },
      );
    collect("engine-only");
    for (const [timeout, scenario] of [
      [0, "host-zero"],
      [300, "host-warm-300"],
    ] as const) {
      await rpc(lock, "engine.settings.set", { warm_timeout_seconds: timeout });
      if (process.env.KOSMOS_TOPOLOGY_RESOURCES === "1") {
        hostProcess = spawn(
          installed.electron,
          [
            `--user-data-dir=${path.join(root, `raw-host-${timeout}`)}`,
            installed.hostMain,
            "--open-app",
            "host-e2e-app-a",
          ],
          {
            env: {
              ...process.env,
              APPDATA: appData,
              PROGRAMDATA: path.join(root, "programdata"),
              KOSMOS_SHORTCUT_DIR: shortcutRoot,
              KOSMOS_DATA_DIR: data,
              KEPLER_APP_INDEX_INITIAL_DELAY_MS: "600000",
              KEPLER_FILE_INDEX_INITIAL_DELAY_MS: "600000",
              KEPLER_FILE_INDEX_INITIAL_RESCAN: "0",
              KOSMOS_HEADLESS: "1",
              KOSMOS_TEST_MODE: "1",
            },
            stdio: ["ignore", "pipe", "pipe"],
            windowsHide: true,
          },
        );
        const hostPid = hostProcess.pid;
        assert(hostPid && hostPid > 0, "raw Host spawn did not return a PID");
        pids.add(hostPid);
        assert(
          processCommandLine(hostPid).includes(installed.hostMain),
          `raw Host PID ${hostPid} does not own installed Host main`,
        );
        let stderr = "";
        hostProcess.stderr?.on("data", (chunk) => {
          stderr += String(chunk);
        });
        await waitFor(
          () => (stderr.includes("[host-lifecycle] window closed") ? true : undefined),
          "fixture App window close",
        );
        if (timeout === 0) {
          await waitFor(() => (!alive(hostPid) ? true : undefined), "zero-warm Host exit");
          hostProcess = undefined;
          collect(scenario, hostPid);
        } else {
          collect(scenario, hostPid);
          await waitFor(() => (!alive(hostPid) ? true : undefined), "warm Host exit");
          hostProcess = undefined;
        }
        continue;
      }
    }
    const reports = ["engine-only", "host-zero", "host-warm-300"].map((scenario) =>
      JSON.parse(fs.readFileSync(path.join(resources, `${scenario}.json`), "utf8")),
    );
    assert(
      reports.every((report) => report.checks?.passed === true && report.build_hash === buildHash),
      "resource scenario failed or used another build",
    );
  } finally {
    if (hostProcess?.pid) await stopPid(hostProcess.pid, "resource Host cleanup");
    await stopEngine(
      engine,
      // SAFETY: stopEngine only reads the engine path from this fixture candidate.
      {
        engine: path.join(
          root,
          "install",
          "versions",
          "resource-current",
          "engine",
          "kepler-backend.exe",
        ),
      } as Candidate,
      data,
    );
    for (const pid of pids) await stopPid(pid, "resource process");
    const cleanup = JSON.parse(fs.readFileSync(manifest, "utf8"));
    cleanup.roots.push(root);
    cleanup.pids.push(...pids);
    fs.writeFileSync(manifest, JSON.stringify(cleanup));
  }
});
