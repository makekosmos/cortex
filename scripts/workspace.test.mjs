import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, utimes, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import {
  inspectDependency,
  doctor,
  cloneExact,
  hash,
  loadWorkspace,
  outputHashes,
  prepareCheckout,
  planBootstrap,
  readStamp,
  resolveMode,
  writeCiOutput,
} from "./workspace.mjs";

const commit = (letter) => letter.repeat(40);

async function fixture() {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-workspace-"));
  await writeFile(
    path.join(root, "package.json"),
    `${JSON.stringify(
      {
        kosmos: {
          workspace: {
            imago: {
              repository: "makekosmos/imago",
              commit: commit("a"),
              package: {
                name: "@makekosmos/visuals",
                version: "0.1.3",
                integrity: `git:${commit("a")}`,
              },
            },
            "arca-sdk": {
              repository: "makekosmos/arca-sdk",
              commit: commit("b"),
              package: {
                name: "@makekosmos/ark",
                version: "0.1.1",
                integrity: `git:${commit("b")}`,
              },
            },
          },
        },
      },
      null,
      2,
    )}\n`,
  );
  return root;
}

test("missing managed checkouts produce clone actions", async () => {
  const root = await fixture();
  const result = await planBootstrap(root, { dryRun: true, prepare: false });
  assert.deepEqual(
    result.actions.map(({ action }) => action),
    ["clone", "clone"],
  );
  assert.equal(result.mutated, false);
});

test("manifest pin mismatches fail closed", async () => {
  const root = await fixture();
  const manifest = JSON.parse(await readFile(path.join(root, "package.json"), "utf8"));
  manifest.kosmos.workspace.imago.commit = commit("c");
  await writeFile(path.join(root, "package.json"), JSON.stringify(manifest));
  await assert.rejects(() => loadWorkspace(root), /integrity must be git:c{40}/);
});

test("matching clean checkouts produce an idempotent no-op", async () => {
  const root = await fixture();
  const workspace = await loadWorkspace(root);
  const manifest = JSON.parse(await readFile(path.join(root, "package.json"), "utf8"));
  for (const [name, pin] of Object.entries(workspace.pins)) {
    const checkout = path.join(root, ".tmp", "workspace", name);
    await mkdir(checkout, { recursive: true });
    execFileSync("git", ["init", "--initial-branch", "main"], { cwd: checkout });
    execFileSync("git", ["config", "user.email", "test@example.invalid"], { cwd: checkout });
    execFileSync("git", ["config", "user.name", "Test"], { cwd: checkout });
    await writeFile(path.join(checkout, "package.json"), JSON.stringify(pin.package));
    execFileSync("git", ["add", "."], { cwd: checkout });
    execFileSync("git", ["commit", "-m", "fixture"], { cwd: checkout });
    const actualCommit = execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: checkout,
      encoding: "utf8",
    }).trim();
    manifest.kosmos.workspace[name].commit = actualCommit;
    manifest.kosmos.workspace[name].package.integrity = `git:${actualCommit}`;
    execFileSync("git", ["checkout", "--detach", actualCommit], { cwd: checkout });
  }
  await writeFile(path.join(root, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  const result = await planBootstrap(root, { dryRun: true, prepare: false });
  assert.deepEqual(
    result.actions.map(({ action }) => action),
    ["noop", "noop"],
  );
});

test("matching bootstrap prepares once, then reuses the content stamp", async () => {
  const root = await fixture();
  const checkout = path.join(root, ".tmp", "workspace", "imago");
  await mkdir(path.join(checkout, "dist"), { recursive: true });
  await writeFile(
    path.join(checkout, "package.json"),
    JSON.stringify({
      name: "@makekosmos/visuals",
      version: "0.1.3",
      packageManager: "bun@1.3.14",
      main: "dist/index.js",
    }),
  );
  await writeFile(path.join(checkout, "bun.lock"), "lock\n");
  await writeFile(path.join(checkout, "dist", "index.js"), "export {}\n");
  await writeFile(path.join(checkout, "dist", "index.css"), ":root {}\n");
  execFileSync("git", ["init", "--initial-branch", "main"], { cwd: checkout });
  execFileSync("git", ["config", "user.email", "test@example.invalid"], { cwd: checkout });
  execFileSync("git", ["config", "user.name", "Test"], { cwd: checkout });
  execFileSync("git", ["add", "."], { cwd: checkout });
  execFileSync("git", ["commit", "-m", "fixture"], { cwd: checkout });
  const actual = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: checkout,
    encoding: "utf8",
  }).trim();
  const manifest = JSON.parse(await readFile(path.join(root, "package.json"), "utf8"));
  manifest.kosmos.workspace.imago.commit = actual;
  manifest.kosmos.workspace.imago.package.integrity = `git:${actual}`;
  await writeFile(path.join(root, "package.json"), JSON.stringify(manifest));
  await mkdir(path.join(root, ".tmp", "workspace", "arca-sdk"), { recursive: true });
  const calls = [];
  const runTool = async (exe, args) => {
    calls.push([exe, ...args]);
    return { status: 0, stdout: args[0] === "--version" ? "1.3.14" : "", stderr: "" };
  };
  const first = await planBootstrap(root, { runTool });
  assert.equal(first.actions.find(({ name }) => name === "imago").action, "prepare");
  assert.deepEqual(calls, [
    ["bun", "--version"],
    ["bun", "install", "--frozen-lockfile"],
    ["bun", "run", "build"],
  ]);
  const second = await planBootstrap(root, { runTool });
  assert.equal(second.actions.find(({ name }) => name === "imago").action, "noop");
  assert.equal(calls.length, 3);
  await assert.rejects(
    () =>
      prepareCheckout("imago", manifest.kosmos.workspace.imago, checkout, async () => ({
        status: 0,
        stdout: "1.3.140",
        stderr: "",
      })),
    /requires bun@1.3.14/,
  );
  const stamp = await readStamp(root);
  stamp.components.imago.outputs["dist/index.css"] = "tampered";
  await writeFile(path.join(root, ".tmp", "workspace", "prepared.json"), JSON.stringify(stamp));
  const inspected = await inspectDependency(
    root,
    "imago",
    checkout,
    manifest.kosmos.workspace.imago,
    stamp,
  );
  assert.equal(inspected.checks.stamp, false);
  assert.equal(
    await hash(path.join(checkout, "dist/index.js")),
    (
      await outputHashes(
        checkout,
        JSON.parse(await readFile(path.join(checkout, "package.json"), "utf8")),
      )
    )["dist/index.js"],
  );
});

test("dirty mismatched checkouts are preserved", async () => {
  const root = await fixture();
  await mkdir(path.join(root, ".tmp", "workspace", "arca-sdk"), { recursive: true });
  await writeFile(path.join(root, ".tmp", "workspace", "arca-sdk", "keep.txt"), "keep\n");
  const checkout = path.join(root, ".tmp", "workspace", "imago");
  await mkdir(checkout, { recursive: true });
  execFileSync("git", ["init", "--initial-branch", "main"], { cwd: checkout });
  execFileSync("git", ["config", "user.email", "test@example.invalid"], { cwd: checkout });
  execFileSync("git", ["config", "user.name", "Test"], { cwd: checkout });
  await writeFile(path.join(checkout, "package.json"), "{}\n");
  execFileSync("git", ["add", "."], { cwd: checkout });
  execFileSync("git", ["commit", "-m", "fixture"], { cwd: checkout });
  await writeFile(path.join(checkout, "dirty.txt"), "keep\n");
  const result = await planBootstrap(root, { dryRun: false, prepare: false });
  const imago = result.actions.find(({ name }) => name === "imago");
  assert.equal(imago.action, "blocked-dirty");
  assert.equal(await readFile(path.join(checkout, "dirty.txt"), "utf8"), "keep\n");
});

test("local mode exposes only explicit overrides", async () => {
  const root = await fixture();
  const paths = {
    imago: path.join(root, "local-imago"),
    "arca-sdk": path.join(root, "local-arca"),
  };
  const mode = resolveMode(
    {
      KOSMOS_WORKSPACE_MODE: "local",
      KOSMOS_IMAGO_PATH: paths.imago,
      KOSMOS_ARCA_SDK_PATH: paths["arca-sdk"],
    },
    root,
  );
  assert.equal(mode.name, "local");
  assert.deepEqual(mode.paths, {
    imago: path.join(root, ".tmp", "workspace", "imago"),
    "arca-sdk": path.join(root, ".tmp", "workspace", "arca-sdk"),
  });
  assert.deepEqual(mode.sources, paths);
  assert.throws(
    () => resolveMode({ KOSMOS_WORKSPACE_MODE: "local", KOSMOS_IMAGO_PATH: paths.imago }),
    /KOSMOS_ARCA_SDK_PATH/,
  );
});

test("local missing paths fail doctor", async () => {
  const root = await fixture();
  const result = await doctor(process.cwd(), {
    env: {
      KOSMOS_WORKSPACE_MODE: "local",
      KOSMOS_IMAGO_PATH: path.join(root, "missing-imago"),
      KOSMOS_ARCA_SDK_PATH: path.join(root, "missing-arca"),
    },
  });
  assert.equal(result.components.imago.bridge.status, "source-missing");
  assert.equal(result.components.imago.bridge.target, path.join(root, "missing-imago"));
});

test("stale dist is reported and CI output uses manifest pins", async () => {
  const root = await fixture();
  const checkout = path.join(root, "imago");
  await mkdir(path.join(checkout, "dist"), { recursive: true });
  await writeFile(path.join(checkout, "index.ts"), "export {}\n");
  await writeFile(path.join(checkout, "dist", "index.js"), "export {}\n");
  await writeFile(
    path.join(checkout, "package.json"),
    JSON.stringify({ name: "@makekosmos/visuals", version: "0.1.3", main: "dist/index.js" }),
  );
  execFileSync("git", ["init", "--initial-branch", "main"], { cwd: checkout });
  execFileSync("git", ["config", "user.email", "test@example.invalid"], { cwd: checkout });
  execFileSync("git", ["config", "user.name", "Test"], { cwd: checkout });
  execFileSync("git", ["add", "."], { cwd: checkout });
  execFileSync("git", ["commit", "-m", "fixture"], { cwd: checkout });
  const old = new Date(0);
  await utimes(path.join(checkout, "dist", "index.js"), old, old);
  const inspected = await inspectDependency(
    root,
    "imago",
    checkout,
    (await loadWorkspace(root)).pins.imago,
  );
  assert.equal(inspected.checks.stamp, undefined);
  const output = path.join(root, "github-output");
  await writeCiOutput(root, output);
  assert.match(await readFile(output, "utf8"), /imago_commit=a{40}/);
  assert.match(await readFile(output, "utf8"), /arca_sdk_commit=b{40}/);
  assert.match(await readFile(output, "utf8"), /imago_repository=makekosmos\/imago/);
  assert.match(await readFile(output, "utf8"), /arca_sdk_repository=makekosmos\/arca-sdk/);
  assert.match(await readFile(output, "utf8"), /imago_path=\.tmp\/workspace\/imago/);
  assert.match(await readFile(output, "utf8"), /arca_sdk_path=\.tmp\/workspace\/arca-sdk/);
});

test("partial clone cleanup removes only the checkout created by this run", async () => {
  const root = await fixture();
  const checkout = path.join(root, ".tmp", "workspace", "imago");
  const pin = (await loadWorkspace(root)).pins.imago;
  const fakeGit = async (cwd, args) => {
    if (args[0] === "clone") await mkdir(checkout, { recursive: true });
    return {
      status: args[0] === "fetch" ? 1 : 0,
      stdout: "",
      stderr: args[0] === "fetch" ? "broken" : "",
    };
  };
  await assert.rejects(
    () =>
      cloneExact(
        root,
        "imago",
        pin,
        checkout,
        async () => ({ status: 0, stdout: "", stderr: "" }),
        { components: {} },
        fakeGit,
      ),
    /fetch imago failed/,
  );
  await assert.rejects(() => access(checkout));
});
