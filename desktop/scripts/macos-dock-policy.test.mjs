import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const cortexRoot = path.resolve(__dirname, "..", "..");

// KOS-376: every executable shipped inside Mundus Manager.app inherits the
// bundle's identity and the default `Regular` activation policy — its first
// WindowServer/AppKit connection registers it with the Dock as a second
// "Mundus Manager" tile. Only the GPUI Manager itself may keep that policy;
// the Engine and every Swift helper must opt out explicitly.

test("Engine marks itself a UIElement process before starting the runtime", async () => {
  const main = await readFile(path.join(cortexRoot, "runtime", "src", "main.rs"), "utf8");
  const suppress = main.indexOf("macos_dock::suppress_dock_tile()");
  assert.notEqual(suppress, -1, "mundus-engine main() must call macos_dock::suppress_dock_tile()");
  const run = main.indexOf("run(args)");
  assert.notEqual(run, -1);
  assert.ok(
    suppress < run,
    "the dock tile must be suppressed before the Engine runtime starts",
  );

  const dock = await readFile(
    path.join(cortexRoot, "runtime", "src", "macos_dock.rs"),
    "utf8",
  );
  assert.match(
    dock,
    /TransformProcessType/,
    "the engine must transform itself into a UIElement (accessory) process",
  );
  assert.match(
    dock,
    /UI_ELEMENT|UIElement|ACCESSORY/i,
    "the transform target must be the accessory/UIElement policy",
  );
});

test("every bundled Swift helper declares a non-Dock activation policy", async () => {
  const helpersDir = path.join(cortexRoot, "runtime", "native", "macos");
  const helpers = (await readdir(helpersDir)).filter((name) => name.endsWith(".swift"));
  assert.ok(helpers.length > 0, "no Swift helpers found");

  const offenders = [];
  for (const name of helpers) {
    const swift = await readFile(path.join(helpersDir, name), "utf8");
    if (!/setActivationPolicy\(\.accessory\)/.test(swift)) {
      offenders.push(name);
    }
  }
  assert.deepEqual(
    offenders,
    [],
    "helpers inside Mundus Manager.app run under the Regular activation " +
      "policy and can claim a second Dock tile on their first WindowServer " +
      "connection; each must call " +
      "NSApplication.shared.setActivationPolicy(.accessory) at startup",
  );
});
