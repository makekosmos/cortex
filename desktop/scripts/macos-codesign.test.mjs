import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { isMachO, listMachOFiles, signingCommands, BUNDLE_ID } from "./macos-codesign.mjs";

test("isMachO detects Mach-O magics and rejects other files", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "mundus-macho-"));
  try {
    const cases = [
      ["thin64", Buffer.from([0xcf, 0xfa, 0xed, 0xfe]), true],
      ["thin32", Buffer.from([0xce, 0xfa, 0xed, 0xfe]), true],
      ["fat", Buffer.from([0xca, 0xfe, 0xba, 0xbe]), true],
      ["fat64", Buffer.from([0xca, 0xfe, 0xba, 0xbf]), true],
      ["plist", Buffer.from("<?xml version"), false],
      ["empty", Buffer.alloc(0), false],
      ["short", Buffer.from([0xcf, 0xfa]), false],
    ];
    for (const [name, content, expected] of cases) {
      const file = path.join(root, name);
      await writeFile(file, content);
      assert.equal(isMachO(file), expected, name);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("listMachOFiles finds every binary under the bundle, not plist/icns", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "mundus-app-"));
  try {
    const macos = path.join(root, "Contents", "MacOS");
    const helpers = path.join(macos, "native", "macos");
    await mkdir(helpers, { recursive: true });
    await mkdir(path.join(root, "Contents", "Resources"), { recursive: true });
    const macho = Buffer.from([0xcf, 0xfa, 0xed, 0xfe, 0, 0, 0, 0]);
    await writeFile(path.join(macos, "Mundus Manager"), macho);
    await writeFile(path.join(macos, "mundus-engine"), macho);
    await writeFile(path.join(helpers, "paste-text"), macho);
    await writeFile(path.join(root, "Contents", "Info.plist"), "<plist/>");
    await writeFile(path.join(root, "Contents", "Resources", "mundus.icns"), "icns");
    const found = listMachOFiles(root)
      .map((f) => path.relative(macos, f))
      .sort();
    assert.deepEqual(found, ["Mundus Manager", "mundus-engine", "native/macos/paste-text"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("ad-hoc signing plan signs every nested Mach-O inside-out, bundle last, no --deep", () => {
  const app = "/tmp/Mundus Manager.app";
  const macos = path.join(app, "Contents", "MacOS");
  const machos = [
    path.join(macos, "Mundus Manager"),
    path.join(macos, "mundus-engine"),
    path.join(macos, "native", "macos", "paste-text"),
  ];
  const commands = signingCommands({ appRoot: app, machos, mainExecutableName: "Mundus Manager" });
  assert.equal(commands.at(-1).target, app, "bundle must be signed last");
  const nested = commands.slice(0, -1);
  assert.equal(nested.length, 2, "nested Mach-Os only — main exe rides the bundle step");
  assert.deepEqual(nested.map((c) => c.target).sort(), [machos[1], machos[2]].sort());
  for (const cmd of commands) {
    assert.ok(!cmd.args.includes("--deep"), "--deep must not appear");
    assert.ok(cmd.args.includes("--timestamp=none"), "ad-hoc needs --timestamp=none");
    assert.ok(cmd.args.includes("-") || cmd.args.includes("--sign"), "ad-hoc sign -");
  }
  const engine = nested.find((c) => c.target.endsWith("mundus-engine"));
  const identIdx = engine.args.indexOf("--identifier");
  assert.notEqual(identIdx, -1);
  assert.equal(engine.args[identIdx + 1], `${BUNDLE_ID}.mundus-engine`);
  const bundleIdent = commands.at(-1).args.indexOf("--identifier");
  assert.equal(commands.at(-1).args[bundleIdent + 1], BUNDLE_ID);
});

test("Developer ID signing plan keeps hardened runtime and timestamp per binary", () => {
  const app = "/tmp/Mundus Manager.app";
  const macos = path.join(app, "Contents", "MacOS");
  const machos = [path.join(macos, "Mundus Manager"), path.join(macos, "mundus-engine")];
  const commands = signingCommands({
    appRoot: app,
    machos,
    mainExecutableName: "Mundus Manager",
    identity: "Developer ID Application: Kazui",
  });
  assert.equal(commands.length, 2);
  for (const cmd of commands) {
    assert.ok(cmd.args.includes("--options"));
    assert.ok(cmd.args.includes("runtime"));
    assert.ok(cmd.args.includes("--timestamp"));
    assert.ok(!cmd.args.includes("--timestamp=none"));
    assert.ok(cmd.args.includes("Developer ID Application: Kazui"));
  }
  assert.equal(commands.at(-1).target, app);
});
