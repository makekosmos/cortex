import { describe, expect, test } from "bun:test";
import type { Catalog } from "../../platform/desktop/electron/extension-marketplace";
import type { InstalledExtensionInfo } from "../../platform/desktop/electron/extension-installer";
import { findExtensionUpdates } from "../../platform/desktop/electron/extension-update-plan";

function installed(
  id: string,
  version: string | null,
  source: InstalledExtensionInfo["source"] = "installed",
): InstalledExtensionInfo {
  return {
    id,
    name: id,
    kind: "vue",
    version,
    description: null,
    author: null,
    iconDataUri: null,
    backupCount: 0,
    backupTimestamps: [],
    source,
  };
}

function catalog(entries: Array<{ id: string; version: string }>): Catalog {
  return {
    schemaVersion: 1,
    updatedAt: "2026-05-30T00:00:00.000Z",
    extensions: entries.map((e) => ({
      id: e.id,
      name: e.id,
      description: "",
      author: null,
      version: e.version,
      keplerApiVersion: "^1.0.0",
      iconUrl: null,
      downloadUrl: `https://example.com/${e.id}-${e.version}.kext`,
      sha256: `${e.id}-sha`,
      size: null,
    })),
  };
}

describe("findExtensionUpdates", () => {
  test("returns only installed extensions with newer catalog versions", () => {
    const result = findExtensionUpdates(
      [installed("eden", "1.0.0"), installed("delphi", "2.0.0"), installed("arrancador", "3.0.0")],
      catalog([
        { id: "eden", version: "1.0.1" },
        { id: "delphi", version: "2.0.0" },
        { id: "arrancador", version: "2.9.9" },
      ]),
    );

    expect(result.map((x) => x.id)).toEqual(["eden"]);
    expect(result[0]!.currentVersion).toBe("1.0.0");
    expect(result[0]!.nextVersion).toBe("1.0.1");
  });

  test("skips dev-source, missing catalog, and invalid semver entries", () => {
    const result = findExtensionUpdates(
      [
        installed("repo-dev", "1.0.0", "dev"),
        installed("missing", "1.0.0"),
        installed("bad-current", "dev"),
        installed("bad-next", "1.0.0"),
      ],
      catalog([
        { id: "repo-dev", version: "1.0.1" },
        { id: "bad-current", version: "1.0.1" },
        { id: "bad-next", version: "next" },
      ]),
    );

    expect(result).toEqual([]);
  });
});
