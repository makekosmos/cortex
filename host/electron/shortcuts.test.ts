import { describe, expect, mock, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const writes: Array<{
  file: string;
  operation: string;
  options: Record<string, unknown>;
}> = [];
mock.module("electron", () => ({
  app: { getPath: () => "unused" },
  shell: {
    writeShortcutLink: (file: string, operation: string, options: Record<string, unknown>) => {
      writes.push({ file, operation, options });
      fs.writeFileSync(file, "shortcut");
      return true;
    },
  },
}));

const { reconcileShortcuts, shortcutArgs } = await import("./shortcuts");

describe("desktop-host shortcut reconciliation", () => {
  test("uses human names, keeps canonical ids in args, and removes migrated entries", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-shortcuts-"));
    process.env.KOSMOS_SHORTCUT_DIR = root;
    writes.length = 0;
    const unrelated = path.join(root, "unrelated.lnk");
    fs.writeFileSync(unrelated, "keep");
    fs.writeFileSync(path.join(root, "notes.app.lnk"), "legacy");
    fs.writeFileSync(path.join(root, "disabled.app.lnk"), "legacy");
    fs.writeFileSync(
      path.join(root, ".kosmos-desktop-host-shortcuts.json"),
      JSON.stringify([
        { id: "notes.app", file: path.join(root, "notes.app.lnk") },
        { id: "disabled.app", file: path.join(root, "disabled.app.lnk") },
      ]),
    );

    reconcileShortcuts(
      [
        { id: "notes.app", name: "Notes", enabled: true },
        { id: "disabled.app", name: "Disabled", enabled: true },
        { id: "../escape", name: "Escape", enabled: true },
      ],
      "host.exe",
    );
    reconcileShortcuts([{ id: "notes.app", name: "Renamed", enabled: true }], "host.exe");

    expect(fs.existsSync(path.join(root, "Renamed.lnk"))).toBe(true);
    expect(fs.existsSync(path.join(root, "notes.app.lnk"))).toBe(false);
    expect(fs.existsSync(path.join(root, "disabled.app.lnk"))).toBe(false);
    expect(writes.every(({ file }) => path.dirname(file) === root)).toBe(true);
    expect(fs.readFileSync(unrelated, "utf8")).toBe("keep");
    expect(writes.at(-1)).toEqual({
      file: path.join(root, "Renamed.lnk"),
      operation: "create",
      options: {
        target: "host.exe",
        args: "--open-app=notes.app",
        description: "Kosmos: Renamed",
      },
    });
    delete process.env.KOSMOS_SHORTCUT_DIR;
    fs.rmSync(root, { recursive: true, force: true });
  });

  test("routes local Electron shortcuts through the installed Host entrypoint", () => {
    const appPath = path.join("C:\\Kosmos Host", "dist-electron", "main.js");
    expect(shortcutArgs(appPath, "notes.app", true)).toBe(`"${appPath}" --open-app=notes.app`);
    expect(shortcutArgs(appPath, "notes.app", false)).toBe("--open-app=notes.app");
  });

  test("uses an app icon when the package provides one", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-shortcuts-icon-"));
    const icon = path.join(root, "eden.png");
    fs.writeFileSync(icon, "icon");
    process.env.KOSMOS_SHORTCUT_DIR = root;
    writes.length = 0;
    reconcileShortcuts(
      [{ id: "com.kosmos.eden", name: "Eden", enabled: true, iconPath: icon }],
      "host.exe",
    );
    expect(writes.at(-1)?.options).toMatchObject({ icon, iconIndex: 0 });
    delete process.env.KOSMOS_SHORTCUT_DIR;
    fs.rmSync(root, { recursive: true, force: true });
  });

  test("sanitizes Windows names and allocates deterministic collisions", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-shortcuts-safe-"));
    process.env.KOSMOS_SHORTCUT_DIR = root;
    writes.length = 0;
    reconcileShortcuts(
      [
        { id: "z.app", name: "Eden", enabled: true },
        { id: "a.app", name: "Eden", enabled: true },
        { id: "con.app", name: "CON", enabled: true },
        { id: "bad.app", name: "bad:/name?", enabled: true },
      ],
      "host.exe",
    );

    expect(writes.map(({ file }) => path.basename(file))).toEqual([
      "Eden (2).lnk",
      "Eden.lnk",
      "_CON.lnk",
      "bad name.lnk",
    ]);
    expect(
      writes.every(({ file }) => path.dirname(path.resolve(file)) === path.resolve(root)),
    ).toBe(true);
    delete process.env.KOSMOS_SHORTCUT_DIR;
    fs.rmSync(root, { recursive: true, force: true });
  });

  test("ignores malformed or unowned index entries", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-shortcuts-"));
    const outsideDir = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-shortcuts-outside-"));
    process.env.KOSMOS_SHORTCUT_DIR = root;
    const outside = path.join(outsideDir, "unrelated.lnk");
    fs.writeFileSync(
      path.join(root, ".kosmos-desktop-host-shortcuts.json"),
      JSON.stringify([
        { id: "../escape", file: outside },
        { id: "other.app", file: outside },
      ]),
    );
    fs.writeFileSync(outside, "keep");

    reconcileShortcuts([], "host.exe");

    expect(fs.existsSync(outside)).toBe(true);
    delete process.env.KOSMOS_SHORTCUT_DIR;
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(outsideDir, { recursive: true, force: true });
  });
});
