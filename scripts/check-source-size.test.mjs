import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const checker = fileURLToPath(new URL("./check-source-size.mjs", import.meta.url));

function lines(count) {
  return Array.from({ length: count }, (_, index) => `// line ${index + 1}`).join("\n");
}

async function runFixture(files) {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-source-size-"));
  try {
    for (const [relative, content] of Object.entries(files)) {
      const target = path.join(root, relative);
      await mkdir(path.dirname(target), { recursive: true });
      await writeFile(target, content, "utf8");
    }
    return spawnSync(process.execPath, [checker, "--root", root], {
      encoding: "utf8",
      windowsHide: true,
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("accepts exactly 500 lines and ignores generated dependencies", async () => {
  const result = await runFixture({
    "src/limit.rs": lines(500),
    "node_modules/generated.mjs": lines(900),
  });
  assert.equal(result.status, 0, result.stderr);
});

test("warns but passes between 300 and 500 lines", async () => {
  const quiet = await runFixture({ "src/small.rs": lines(300) });
  assert.equal(quiet.status, 0, quiet.stderr);
  assert.doesNotMatch(quiet.stderr, /warning/);

  const result = await runFixture({ "src/medium.mjs": lines(301) });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /source size warning: 1 file/);
  assert.match(result.stderr, /medium\.mjs: 301 lines \(aim for 300\)/);
});

test("rejects a new source file above 500 lines", async () => {
  const result = await runFixture({ "src/new-module.mjs": lines(501) });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /new-module\.mjs: 501 lines \(max 500\)/);
});

test("retired Godfile paths cannot reuse an old exemption", async () => {
  const result = await runFixture({ "runtime/src/integrations.rs": lines(501) });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /runtime\/src\/integrations\.rs: 501 lines/);
});

test("core/ is first-party and checked like everything else", async () => {
  const result = await runFixture({ "core/crates/ark-core/src/big.rs": lines(501) });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /core\/crates\/ark-core\/src\/big\.rs: 501 lines \(max 500\)/);
});

test("only the explicit debt baseline is grandfathered", async () => {
  const result = await runFixture({
    "runtime/src/main.rs": lines(501),
    "runtime/src/package_store.rs": lines(501),
    "runtime/src/focus.rs": lines(501),
  });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /3 grandfathered file/);
});

test("pruned baseline entries under 500 lines are no longer exempt", async () => {
  const result = await runFixture({ "runtime/src/ark_host.rs": lines(501) });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /runtime\/src\/ark_host\.rs: 501 lines \(max 500\)/);
});

test("moved packages/ sources are no longer grandfathered", async () => {
  const result = await runFixture({
    "packages/huawei-health/src/lib.rs": lines(565),
    "runtime/src/package_service/integrations/huawei_login.rs": lines(541),
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /packages\/huawei-health\/src\/lib\.rs: 565 lines \(max 500\)/);
  assert.match(result.stderr, /huawei_login\.rs: 541 lines \(max 500\)/);
});
