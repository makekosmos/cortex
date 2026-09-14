import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";

const root = path.resolve(import.meta.dirname, "..");
const packageRoots = [".", "desktop", "host", "manager"];
const read = (file) => readFileSync(path.join(root, file), "utf8");
const require = createRequire(path.join(root, "desktop", "package.json"));
const typescript = require("typescript");

test("Cortex uses pnpm 12.4.1 for every package root", () => {
  for (const packageRoot of packageRoots) {
    const relative = path.join(packageRoot, "package.json");
    const packageJson = JSON.parse(read(relative));
    assert.equal(packageJson.packageManager, "pnpm@12.4.1", relative);
    assert.equal(existsSync(path.join(root, packageRoot, "bun.lock")), false, relative);
    assert.equal(existsSync(path.join(root, packageRoot, "pnpm-lock.yaml")), true, relative);
  }
});

test("Cortex command and test contracts do not require Bun", () => {
  const files = [
    "package.json",
    "desktop/package.json",
    "host/package.json",
    "manager/package.json",
    "lefthook.yml",
    ".github/workflows/ci.yml",
    "README.md",
  ];
  for (const file of files) assert.doesNotMatch(read(file), /\bbun(?:x)?\b|bun:test/, file);
  for (const file of ["desktop", "host", "manager"]) {
    const contents = read(`${file}/package.json`);
    assert.doesNotMatch(contents, /\bbun(?:x)?\b|bun:test/, `${file}/package.json`);
  }
});

test("release BOM records the pnpm toolchain without changing Rust commands", () => {
  const source = read("desktop/scripts/release-bom.mjs");
  assert.match(source, /pnpm/);
  assert.doesNotMatch(source, /\bbun\b/);
  assert.match(read("package.json"), /cargo fmt --all -- --check/);
  assert.match(read("package.json"), /cargo clippy --workspace --all-targets/);
  assert.match(read("package.json"), /test:rust/);
});

test("Node-launched host E2E uses pnpm exec", () => {
  const source = read("host/scripts/run-e2e.mjs");
  assert.match(source, /createRequire\(import\.meta\.url\)\.resolve\("@playwright\/test\/cli"\)/);
  assert.match(source, /process\.execPath,\s*\[playwrightCli, "test"/);
  assert.doesNotMatch(source, /shell:\s*true/);
});

test("CI pins pnpm action setup to the reviewed v4 commit", () => {
  const source = read(".github/workflows/ci.yml");
  assert.equal(
    (source.match(/pnpm\/action-setup@f40ffcd9367d9f12939873eb1018b921a783ffaa/g) ?? []).length,
    3,
  );
  assert.doesNotMatch(source, /pnpm\/action-setup@v4/);
});

test("Desktop Vue uses the workspace-pinned shared Imago revision", () => {
  const desktop = JSON.parse(read("desktop/package.json"));
  assert.equal(desktop.devDependencies.vue, "3.6.0-rc.7");
  const ci = read(".github/workflows/ci.yml");
  assert.match(ci, /repository: \$\{\{ steps\.workspace\.outputs\.imago_repository \}\}/);
  assert.match(ci, /ref: \$\{\{ steps\.workspace\.outputs\.imago_commit \}\}/);
  assert.match(ci, /path: \.tmp\/workspace\/imago/);
});

test("Desktop and pinned Imago resolve Vue to one type identity", () => {
  const compilerOptions = typescript.parseJsonConfigFileContent(
    JSON.parse(read("desktop/tsconfig.json")),
    typescript.sys,
    path.join(root, "desktop"),
  ).options;
  const resolveVue = (containingFile) => {
    const resolved = typescript.resolveModuleName(
      "vue",
      containingFile,
      compilerOptions,
      typescript.sys,
    ).resolvedModule?.resolvedFileName;
    assert.ok(resolved, `Vue did not resolve from ${containingFile}`);
    return require("node:fs").realpathSync(resolved);
  };
  assert.equal(
    resolveVue(path.join(root, "desktop/src/main.ts")),
    resolveVue(path.join(root, ".tmp/workspace/imago/index.ts")),
  );
});
