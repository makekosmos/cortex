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

test("accepts exactly 300 lines and ignores generated dependencies", async () => {
  const result = await runFixture({
    "src/limit.rs": lines(300),
    "node_modules/generated.mjs": lines(600),
  });
  assert.equal(result.status, 0, result.stderr);
});

test("rejects a new source file above 300 lines", async () => {
  const result = await runFixture({ "src/new-module.mjs": lines(301) });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /new-module\.mjs: 301 lines \(max 300\)/);
});

test("retired Godfile paths cannot reuse an old exemption", async () => {
  const result = await runFixture({ "runtime/src/integrations.rs": lines(301) });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /runtime\/src\/integrations\.rs: 301 lines/);
});

test("only the explicit debt baseline is grandfathered", async () => {
  const result = await runFixture({
    "packages/huawei-health/src/lib.rs": lines(301),
    "packages/huawei-health/tests/archive.rs": lines(301),
    "runtime/src/main.rs": lines(301),
    "runtime/src/package_service/integrations.rs": lines(301),
    "runtime/src/package_service/integrations/huawei_login.rs": lines(301),
    "runtime/src/package_worker_supervisor/authority.rs": lines(301),
    "runtime/src/package_worker_supervisor/calls_dispatch.rs": lines(301),
    "runtime/src/package_worker_supervisor/tests/api/opaque_roots.rs": lines(301),
  });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /8 grandfathered file/);
});

test("grandfathers only the current Huawei source-size debt", async () => {
  const result = await runFixture({
    "packages/huawei-health/src/lib.rs": lines(365),
    "packages/huawei-health/tests/archive.rs": lines(443),
    "runtime/src/package_service/integrations/huawei_login.rs": lines(541),
  });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /3 grandfathered file/);
});
