import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  type PathResolutionContext,
  resolveManifestRoots,
  resolveRootPathPattern,
} from "./path-expansion";

const tempRoots: string[] = [];

async function createTempRoot() {
  const root = await mkdtemp(path.join(tmpdir(), "arrancador-path-expansion-"));
  tempRoots.push(root);
  return root;
}

function createContext(root: string): PathResolutionContext {
  return {
    home: root,
    documents: path.join(root, "Documents"),
    appdata: null,
    localAppData: null,
    localLow: null,
    savedGames: null,
    publicPath: null,
    publicDocuments: null,
    programData: null,
    steam: null,
    steamUserData: null,
  };
}

afterEach(async () => {
  await Promise.all(
    tempRoots.splice(0).map((root) =>
      rm(root, {
        recursive: true,
        force: true,
      }),
    ),
  );
});

describe("backup path expansion", () => {
  it("expands known manifest tokens and ignores unavailable token roots", async () => {
    const root = await createTempRoot();
    const documents = path.join(root, "Documents");
    const saves = path.join(documents, "My Games", "Control");
    await mkdir(saves, { recursive: true });

    await expect(
      resolveRootPathPattern("<winDocuments>\\My Games\\Control", createContext(root)),
    ).resolves.toEqual([saves]);

    await expect(
      resolveRootPathPattern("<steamUserData>\\*", createContext(root)),
    ).resolves.toEqual([]);
  });

  it("resolves all file roots from a manifest entry", async () => {
    const root = await createTempRoot();
    const control = path.join(root, "Documents", "Control");
    const settings = path.join(root, "Documents", "Control Settings");
    await mkdir(control, { recursive: true });
    await mkdir(settings, { recursive: true });

    const roots = await resolveManifestRoots(
      {
        files: {
          saves: ["<documents>\\Control"],
          settings: ["<documents>\\Control Settings"],
        },
      },
      createContext(root),
    );

    expect(roots.sort()).toEqual([control, settings].sort());
  });
});
