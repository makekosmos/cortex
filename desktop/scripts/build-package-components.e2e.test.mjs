import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";

const root = path.resolve(import.meta.dirname, "..", "..");
const desktop = path.join(root, "desktop");

// This exercises the component build itself, so it runs with --local: the
// release preflight (clean main, publishable version, Engine manifest) has its
// own tests and must not be satisfied here with fabricated inputs.
// The Manager component is manager-gpui (cargo); only the desktop leg needs
// installed pnpm dependencies.
const prerequisites =
  process.platform === "win32" &&
  existsSync(path.join(desktop, "node_modules")) &&
  spawnSync("cargo", ["--version"], { encoding: "utf8" }).status === 0;

test(
  "build-package-components.mjs builds the icons and the Manager component",
  {
    timeout: 15 * 60_000,
    skip: prerequisites ? false : "requires Windows, installed desktop deps, and cargo on PATH",
  },
  async () => {
    const result = spawnSync(
      process.execPath,
      [path.join(desktop, "scripts", "build-package-components.mjs"), "--local"],
      {
        cwd: desktop,
        env: process.env,
        encoding: "utf8",
        timeout: 14 * 60_000,
      },
    );
    assert.equal(
      result.status,
      0,
      `exit ${result.status} ${result.error?.code ?? ""}\n${result.stderr}\n${result.stdout.slice(-3000)}`,
    );

    for (const name of ["mundus", "memoria", "agenda", "dictation"])
      assert.ok(existsSync(path.join(desktop, "build", "app-icons", `${name}.ico`)), name);
    // Only Manager is staged as a bundled component — assert on the staged
    // tree, not on the script text.
    const componentsDir = path.join(desktop, ".tmp", "components");
    assert.deepEqual(
      readdirSync(componentsDir).sort(),
      ["manager"],
      "components staging must contain only manager",
    );
    const managerOut = path.join(componentsDir, "manager", "win-unpacked");
    assert.ok(existsSync(path.join(managerOut, "Mundus Manager.exe")), "manager unpackaged output");
    assert.ok(
      !existsSync(path.join(managerOut, "resources")),
      "manager component must be the GPUI exe, not an Electron package",
    );
  },
);
