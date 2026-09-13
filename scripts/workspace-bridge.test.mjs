import assert from "node:assert/strict";
import { access, mkdir, mkdtemp, readFile, realpath, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { planBootstrap, resolveMode } from "./workspace.mjs";

const sha = (letter) => letter.repeat(40);
async function fixture() {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-bridge-"));
  const component = (repository, letter, name, version) => ({
    repository,
    commit: sha(letter),
    package: { name, version, integrity: `git:${sha(letter)}` },
  });
  await writeFile(
    path.join(root, "package.json"),
    JSON.stringify({
      kosmos: {
        workspace: {
          imago: component("makekosmos/imago", "a", "@makekosmos/visuals", "0.1.3"),
          "arca-sdk": component("makekosmos/arca-sdk", "b", "@makekosmos/ark", "0.1.1"),
        },
      },
    }),
  );
  return root;
}
const localEnv = (root) => ({
  KOSMOS_WORKSPACE_MODE: "local",
  KOSMOS_IMAGO_PATH: path.join(root, "local-imago"),
  KOSMOS_ARCA_SDK_PATH: path.join(root, "local-arca"),
});

test("local bootstrap creates managed bridges and repeats as noop", async () => {
  const root = await fixture(),
    env = localEnv(root);
  await Promise.all([mkdir(env.KOSMOS_IMAGO_PATH), mkdir(env.KOSMOS_ARCA_SDK_PATH)]);
  const first = await planBootstrap(root, { env }),
    mode = resolveMode(env, root);
  assert.deepEqual(
    first.actions.map(({ action }) => action),
    ["link", "link"],
  );
  assert.equal(first.mutated, true);
  for (const name of Object.keys(mode.paths))
    assert.equal(await realpath(mode.paths[name]), await realpath(mode.sources[name]));
  const second = await planBootstrap(root, { env });
  assert.deepEqual(
    second.actions.map(({ action }) => action),
    ["noop", "noop"],
  );
  assert.equal(second.mutated, false);
});

test("local bootstrap dry-run plans missing bridges", async () => {
  const root = await fixture(),
    env = localEnv(root);
  await Promise.all([mkdir(env.KOSMOS_IMAGO_PATH), mkdir(env.KOSMOS_ARCA_SDK_PATH)]);
  const result = await planBootstrap(root, { env, dryRun: true });
  assert.deepEqual(
    result.actions.map(({ action }) => action),
    ["link", "link"],
  );
  assert.equal(result.mutated, false);
  await assert.rejects(() => access(path.join(root, ".tmp", "workspace")));
});

test("local bootstrap preserves wrong links and existing managed directories", async () => {
  const root = await fixture(),
    env = localEnv(root),
    mode = resolveMode(env, root);
  await Promise.all([mkdir(env.KOSMOS_IMAGO_PATH), mkdir(env.KOSMOS_ARCA_SDK_PATH)]);
  const wrong = path.join(root, "wrong-imago");
  await mkdir(wrong);
  await mkdir(path.dirname(mode.paths.imago), { recursive: true });
  await symlink(wrong, mode.paths.imago, process.platform === "win32" ? "junction" : "dir");
  await mkdir(mode.paths["arca-sdk"]);
  await writeFile(path.join(mode.paths["arca-sdk"], "keep.txt"), "keep\n");
  const result = await planBootstrap(root, { env });
  assert.deepEqual(
    result.actions.map(({ action }) => action),
    ["blocked-link", "blocked-existing"],
  );
  assert.equal(await realpath(mode.paths.imago), await realpath(wrong));
  assert.equal(await readFile(path.join(mode.paths["arca-sdk"], "keep.txt"), "utf8"), "keep\n");
  assert.equal(result.mutated, false);
});
