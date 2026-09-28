import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { repositoryContext } from "./release-bom.mjs";
import { documentHash } from "./package-release-utils.mjs";

const root = path.resolve(import.meta.dirname, "..", "..");
const desktop = path.join(root, "desktop");
const git = (args) => spawnSync("git", args, { cwd: root, encoding: "utf8" }).stdout?.trim() ?? "";

// The release pipeline refuses to run on a dirty tree; a fabricatable BOM is
// the only other prerequisite, so a clean committed checkout is required.
const cleanWorktree =
  git(["status", "--porcelain", "--untracked-files=no"]) === "" &&
  git([
    "ls-files",
    "--others",
    "--exclude-standard",
    "--",
    "desktop/scripts",
    "desktop/build",
    "manager-gpui",
    "runtime/src",
    "native-services",
  ]) === "";

// The Manager component is manager-gpui (cargo); only the desktop leg needs
// installed pnpm dependencies.
const dependenciesReady = existsSync(path.join(desktop, "node_modules"));

function componentReady(pinKey, envVar, sibling) {
  const pin = JSON.parse(readFileSync(path.join(desktop, "component-pins.json"), "utf8"))[pinKey];
  const source = path.resolve(process.env[envVar]?.trim() || path.join(root, "..", sibling));
  return (
    !!pin?.commit &&
    existsSync(path.join(source, "Cargo.toml")) &&
    spawnSync("git", ["rev-parse", "HEAD"], { cwd: source, encoding: "utf8" }).stdout?.trim() ===
      pin.commit
  );
}

const prerequisites =
  process.platform === "win32" &&
  cleanWorktree &&
  dependenciesReady &&
  componentReady("agenda_gpui", "KOSMOS_AGENDA_GPUI_SRC", "agenda-gpui") &&
  componentReady("memoria_gpui", "KOSMOS_MEMORIA_GPUI_SRC", "memoria-gpui") &&
  componentReady("dictation_gpui", "KOSMOS_DICTATION_GPUI_SRC", "dictation") &&
  spawnSync("cargo", ["--version"], { encoding: "utf8" }).status === 0 &&
  git(["rev-parse", "HEAD"]);

async function writeTestBom() {
  const currentCommit = git(["rev-parse", "HEAD"]);
  const ctx = await repositoryContext(root, "win", currentCommit);
  assert.equal(
    ctx.currentCommit,
    currentCommit,
    "release requires the in-tree core/ subtree to ride on the Cortex commit",
  );

  const arkDir = path.join(desktop, ".tmp", "runtime.next");
  mkdirSync(arkDir, { recursive: true });
  const arkFile = path.join(arkDir, "ark-core-rpc.exe");
  const arkBytes = Buffer.from(`kosmos e2e ark artifact ${currentCommit}\n`);
  writeFileSync(arkFile, arkBytes);
  const engineDir = path.join(desktop, ".tmp", "engine.next");
  mkdirSync(engineDir, { recursive: true });
  writeFileSync(path.join(engineDir, "engine-manifest.json"), JSON.stringify({ version: "0.1.0" }));

  const bom = {
    schema_version: 1,
    state: "resolved",
    id: "e2e-components",
    release: { channel: "e2e", platform: "win", version: ctx.releaseVersion },
    source: {
      cortex: { repository: "makekosmos/cortex", commit: currentCommit },
      core: {
        repository: "makekosmos/cortex",
        commit: currentCommit,
        path: "core/",
        ark_artifact: {
          name: "ark-core-rpc.exe",
          sha256: documentHash(arkBytes),
          size: arkBytes.length,
        },
      },
      arca_sdk: ctx.workspace["arca-sdk"],
      imago: ctx.workspace.imago,
      store: { repository: "makekosmos/store", commit: currentCommit },
      toolchain: { ...ctx.toolchain, target: "x86_64-pc-windows-msvc" },
    },
    compatibility: {
      engine_api: ctx.api.engine,
      package_schema: ctx.api.package_manifest,
    },
    catalog: {
      sequence: 1,
      previous_sequence: 0,
      store_sequence: 1,
      signing_key_id: "e2e-signing",
    },
    packages: [
      {
        id: "e2e-package",
        manifest_id: "e2e-package",
        kind: "source",
        repository: "makekosmos/cortex",
        ref: currentCommit,
        version: "0.1.0",
        entrypoint: "index.ts",
        icon: "icon.png",
        engine_api: ctx.api.engine,
        artifact: {
          name: "e2e-package.kspkg",
          url: "https://github.com/makekosmos/package-index/releases/download/catalog-1/e2e-package.kspkg",
          sha256: documentHash(Buffer.from("e2e")),
          size: 3,
        },
      },
    ],
  };
  const file = path.join(await mkdtemp(path.join(os.tmpdir(), "kosmos-bom-")), "bom.json");
  await writeFile(file, JSON.stringify(bom, null, 2));
  return file;
}

test(
  "build-package-components.mjs completes icons and GPUI component packaging",
  {
    timeout: 15 * 60_000,
    skip: prerequisites
      ? false
      : "requires Windows, a clean committed worktree, installed desktop deps, cargo on PATH, and pinned component checkouts",
  },
  async () => {
    const bom = await writeTestBom();
    const result = spawnSync(
      process.execPath,
      [path.join(desktop, "scripts", "build-package-components.mjs")],
      {
        cwd: desktop,
        env: { ...process.env, KOSMOS_RELEASE_BOM: bom },
        encoding: "utf8",
        timeout: 14 * 60_000,
      },
    );
    assert.equal(
      result.status,
      0,
      `exit ${result.status} ${result.error?.code ?? ""}\n${result.stderr}\n${result.stdout.slice(-3000)}`,
    );

    for (const name of ["kosmos", "memoria", "agenda", "arcadia", "dictation"])
      assert.ok(existsSync(path.join(desktop, "build", "app-icons", `${name}.ico`)), name);
    for (const [component, exe] of [
      ["manager", "Kosmos Manager.exe"],
      ["agenda", "Kosmos Agenda.exe"],
      ["memoria", "Kosmos Memoria.exe"],
      ["dictation", "Kosmos Dictation.exe"],
    ]) {
      const out = path.join(desktop, ".tmp", "components", component, "win-unpacked");
      assert.ok(existsSync(path.join(out, exe)), `${component} unpackaged output`);
      assert.ok(
        !existsSync(path.join(out, "resources")),
        `${component} component must be the GPUI exe, not an Electron package`,
      );
    }
  },
);
