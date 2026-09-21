import assert from "node:assert/strict";
import { mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { documentHash } from "./package-release-utils.mjs";
import { loadReleaseBom, validateReleaseBom } from "./release-bom.mjs";

const context = {
  currentCommit: "a".repeat(40),
  platform: "win",
  releaseVersion: "0.9.15",
  workspace: {
    imago: {
      repository: "makekosmos/imago",
      commit: "d".repeat(40),
      package: {
        name: "@makekosmos/visuals",
        version: "0.1.3",
        integrity: `git:${"d".repeat(40)}`,
      },
    },
    "arca-sdk": {
      repository: "makekosmos/arca-sdk",
      commit: "c".repeat(40),
      package: { name: "@makekosmos/ark", version: "0.1.1", integrity: `git:${"c".repeat(40)}` },
    },
  },
  toolchain: { pnpm: "12.4.1", node: "24.15.0", rust: "1.95.0" },
  api: { shell: "1.1.0", engine: "1.0.0", package_manifest: 2 },
};

function bom() {
  return {
    schema_version: 1,
    state: "resolved",
    id: "cortex-0.9.15-win",
    release: { channel: "stable", platform: "win", version: "0.9.15" },
    source: {
      cortex: { repository: "makekosmos/cortex", commit: context.currentCommit },
      core: {
        repository: "makekosmos/cortex",
        commit: context.currentCommit,
        path: "core/",
        ark_artifact: { name: "ark-core-rpc.exe", sha256: "a".repeat(64), size: 9454592 },
      },
      arca_sdk: {
        repository: "makekosmos/arca-sdk",
        commit: "c".repeat(40),
        package: { name: "@makekosmos/ark", version: "0.1.1", integrity: `git:${"c".repeat(40)}` },
      },
      imago: {
        repository: "makekosmos/imago",
        commit: "d".repeat(40),
        package: {
          name: "@makekosmos/visuals",
          version: "0.1.3",
          integrity: `git:${"d".repeat(40)}`,
        },
      },
      store: { repository: "makekosmos/store", commit: "f".repeat(40) },
      toolchain: { ...context.toolchain, target: "x86_64-pc-windows-msvc" },
    },
    compatibility: {
      shell_api: context.api.shell,
      engine_api: context.api.engine,
      package_schema: context.api.package_manifest,
    },
    catalog: {
      sequence: 12,
      previous_sequence: 11,
      store_sequence: 12,
      channel: "production",
      signing_key_id: "release",
    },
    packages: [
      {
        id: "com.kosmos.fixture",
        manifest_id: "com.kosmos.fixture",
        kind: "app",
        repository: "makekosmos/fixture",
        version: "1.0.0",
        ref: "e".repeat(40),
        engine_api: ">=1.0.0",
        entrypoint: "dist/index.html",
        icon: "icon.png",
        artifact: {
          name: "fixture.kspkg",
          url: "https://github.com/makekosmos/package-index/releases/download/catalog-12/fixture.kspkg",
          sha256: "f".repeat(64),
          size: 1,
        },
      },
    ],
    artifacts: [],
  };
}

test("validates the pinned Cortex release BOM", () => {
  assert.equal(validateReleaseBom(bom(), context).id, "cortex-0.9.15-win");
});

test("rejects signing secrets", () => {
  const value = bom();
  value.signing = { private_key: "never" };
  assert.throws(() => validateReleaseBom(value, context), /private_key/);
});

test("rejects mutable package URLs and a divergent Core subtree commit", () => {
  const mutable = bom();
  mutable.packages[0].artifact.url =
    "https://github.com/makekosmos/package-index/releases/latest/download/fixture.kspkg";
  assert.throws(() => validateReleaseBom(mutable, context), /immutable catalog release/);
  const divergent = bom();
  divergent.source.core.commit = "b".repeat(40);
  assert.throws(() => validateReleaseBom(divergent, context), /source\.core\.commit/);
});

test("rejects first-party source metadata outside package.json pins", () => {
  const value = bom();
  value.source.imago.package.version = "0.1.4";
  assert.throws(() => validateReleaseBom(value, context), /package\.json workspace pin/);
});

for (const [name, edit, message] of [
  [
    "Cortex commit",
    (value) => (value.source.cortex.commit = "f".repeat(40)),
    "source.cortex.commit",
  ],
  ["release version", (value) => (value.release.version = "0.9.16"), "release.version"],
  ["toolchain", (value) => (value.source.toolchain.rust = "1.94.0"), "source.toolchain.rust"],
  ["API pin", (value) => (value.compatibility.engine_api = "2.0.0"), "compatibility.engine_api"],
]) {
  test(`rejects a mismatched ${name}`, () => {
    const value = bom();
    edit(value);
    assert.throws(() => validateReleaseBom(value, context), new RegExp(message));
  });
}

test("loads exact BOM bytes and digest", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "cortex-bom-"));
  const file = path.join(dir, "release-bom.json");
  const raw = Buffer.from(`${JSON.stringify(bom(), null, 2)}\n`);
  await writeFile(file, raw);
  const loaded = await loadReleaseBom(file, { context });
  assert.equal(loaded.bytes.toString(), raw.toString());
  assert.equal(loaded.digest, documentHash(raw));
});
