import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, chmod, rm, readFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { dmgName, MANAGER_MAC_BIN, ENGINE_BINARY_UNIX } from "./brand.mjs";

test("dmgName is Mundus-<ver>.dmg", () => {
  assert.equal(dmgName("1.2.3"), "Mundus-1.2.3.dmg");
});

test("stageMacosApp lays out Engine + Manager + helpers for lookup", async () => {
  const { stageMacosApp } = await import("./package-macos-dmg.mjs");
  const root = await mkdtemp(path.join(os.tmpdir(), "mundus-dmg-"));
  const previous = process.env.CARGO_TARGET_DIR;
  try {
    const cargoTarget = path.join(root, "cargo-target");
    await mkdir(path.join(cargoTarget, "release"), { recursive: true });
    const engine = path.join(cargoTarget, "release", ENGINE_BINARY_UNIX);
    const manager = path.join(cargoTarget, "release", "manager-gpui");
    await writeFile(engine, "engine-bin");
    await writeFile(manager, "manager-bin");
    await chmod(engine, 0o755);
    await chmod(manager, 0o755);
    process.env.CARGO_TARGET_DIR = cargoTarget;

    // Helpers under desktop/.tmp relative to package script — point via real tree.
    // stageMacosApp reads desktop/.tmp/native/macos from the repo; create there
    // only if missing and clean up after.
    const helpersDir = path.resolve(import.meta.dirname, "..", ".tmp", "native", "macos");
    await mkdir(helpersDir, { recursive: true });
    const helper = path.join(helpersDir, "hotkey-hold-monitor");
    const createdHelper = !existsSync(helper);
    if (createdHelper) await writeFile(helper, "helper");

    const stageDir = path.join(root, "stage");
    const appPath = stageMacosApp({ version: "1.2.3", stageDir });
    assert.ok(appPath.endsWith("Mundus Manager.app"));
    assert.equal(existsSync(path.join(appPath, "Contents", "MacOS", MANAGER_MAC_BIN)), true);
    assert.equal(existsSync(path.join(appPath, "Contents", "MacOS", ENGINE_BINARY_UNIX)), true);
    assert.equal(
      existsSync(path.join(appPath, "Contents", "MacOS", "native", "macos", "hotkey-hold-monitor")),
      true,
    );
    const plist = await readFile(path.join(appPath, "Contents", "Info.plist"), "utf8");
    assert.match(plist, /<key>CFBundleIconFile<\/key>\s*<string>mundus<\/string>/);
    if (process.platform === "darwin") {
      assert.equal(existsSync(path.join(appPath, "Contents", "Resources", "mundus.icns")), true);
    }
    if (createdHelper) await rm(helper, { force: true });
  } finally {
    if (previous === undefined) delete process.env.CARGO_TARGET_DIR;
    else process.env.CARGO_TARGET_DIR = previous;
    await rm(root, { recursive: true, force: true });
  }
});

test("createUnsignedDmg refuses non-darwin hosts", async () => {
  if (process.platform === "darwin") return;
  const { createUnsignedDmg } = await import("./package-macos-dmg.mjs");
  assert.throws(
    () => createUnsignedDmg({ appPath: "/tmp/x.app", dmgPath: "/tmp/x.dmg", volumeName: "x" }),
    /requires macOS/,
  );
});

test("dmgbuild settings carry the drag-to-Applications layout", async () => {
  const source = await readFile(path.join(import.meta.dirname, "package-macos-dmg.mjs"), "utf8");
  // Applications symlink, volume icon, and hidpi background are what turn the
  // raw image into the classic install window — guard them in the settings.
  assert.match(source, /"symlinks":\s*\{\s*"Applications":\s*"\/Applications"\s*\}/);
  assert.match(source, /"icon":.*mundus\.icns/);
  assert.match(source, /"background":/);
  assert.match(source, /"icon_locations":\s*\{[^}]*"Applications":\s*\(495,\s*195\)/);
});
