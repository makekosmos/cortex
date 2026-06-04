import { describe, expect, test } from "bun:test";
import { parseRaycastPackageManifest } from "../../shell/electron/raycast/manifest";

describe("Raycast package manifest parser", () => {
  test("parses Raycast package.json metadata, commands, preferences, and kosmos namespace", () => {
    const manifest = parseRaycastPackageManifest({
      name: "notes-tools",
      title: "Notes Tools",
      version: "1.2.3",
      description: "Tools for notes",
      author: { name: "Kazui" },
      preferences: [{ name: "token", type: "password", default: "dev" }],
      commands: [
        {
          name: "copy",
          title: "Copy",
          subtitle: "Clipboard",
          mode: "no-view",
          keywords: ["clipboard"],
          preferences: [{ name: "prefix", type: "textfield", default: "Note" }],
        },
      ],
      kosmos: {
        permissions: ["userData.read", "userData.write"],
        windowEffect: "mica",
        minKosmosApiVersion: "^1.0.0",
        commands: {
          copy: { entry: "dist/copy.mjs" },
        },
      },
    });

    expect(manifest?.name).toBe("notes-tools");
    expect(manifest?.title).toBe("Notes Tools");
    expect(manifest?.author).toBe("Kazui");
    expect(manifest?.commands[0]?.mode).toBe("no-view");
    expect(manifest?.commands[0]?.keywords).toEqual(["clipboard"]);
    expect(manifest?.preferences?.[0]?.default).toBe("dev");
    expect(manifest?.kosmos?.permissions).toEqual(["userData.read", "userData.write"]);
    expect(manifest?.kosmos?.commands?.copy?.entry).toBe("dist/copy.mjs");
  });

  test("rejects packages without Raycast commands", () => {
    expect(parseRaycastPackageManifest({ name: "plain-package" })).toBeNull();
  });

  test("defaults missing command mode to view and skips explicit unknown modes", () => {
    const manifest = parseRaycastPackageManifest({
      name: "mode-tools",
      commands: [
        { name: "defaulted", title: "Defaulted" },
        { name: "status", title: "Status", mode: "menu-bar" },
        { name: "bad", title: "Bad", mode: "background" },
      ],
    });

    expect(manifest?.commands.map((command) => [command.name, command.mode])).toEqual([
      ["defaulted", "view"],
      ["status", "menu-bar"],
    ]);
  });

  test("rejects packages with only unknown command modes", () => {
    expect(
      parseRaycastPackageManifest({
        name: "bad-tools",
        commands: [{ name: "bad", title: "Bad", mode: "background" }],
      }),
    ).toBeNull();
  });
});
