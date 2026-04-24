import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  findGameSaves,
  findSavePath,
  resolveSavePathTemplate,
  tokenizeSavePath,
} from "./save-locator";

const tempRoots: string[] = [];

async function createTempRoot() {
  const root = await mkdtemp(path.join(tmpdir(), "arrancador-save-locator-"));
  tempRoots.push(root);
  return root;
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

describe("save locator", () => {
  it("discovers files and total size from an explicit override root", async () => {
    const root = await createTempRoot();
    const nested = path.join(root, "profile");
    await mkdir(nested);
    await writeFile(path.join(root, "settings.sav"), "abc");
    await writeFile(path.join(nested, "slot1.sav"), "12345");

    const discovery = await findGameSaves("Example Game", null, root);

    expect(discovery?.roots).toEqual([{ label: "root-0", path: root }]);
    expect(discovery?.totalSize).toBe(8);
    expect(discovery?.files.map((file) => file.relativePath).sort()).toEqual([
      "profile\\slot1.sav",
      "settings.sav",
    ]);
  });

  it("returns a single explicit save path candidate", async () => {
    const root = await createTempRoot();

    const result = await findSavePath({
      gameName: "Example Game",
      overridePath: root,
      manifest: null,
    });

    expect(result).toEqual({
      savePath: root,
      candidates: [root],
    });
  });

  it("resolves and tokenizes save paths relative to the game executable", async () => {
    const root = await createTempRoot();
    const gameDir = path.join(root, "Game");
    const saveDir = path.join(gameDir, "Saves");
    const exePath = path.join(gameDir, "game.exe");
    await mkdir(saveDir, { recursive: true });
    await writeFile(exePath, "");

    await expect(
      resolveSavePathTemplate(exePath, "{PATHTOGAME}\\Saves"),
    ).resolves.toBe(saveDir);
    await expect(tokenizeSavePath(exePath, saveDir)).resolves.toBe(
      "{PATHTOGAME}\\Saves",
    );
  });
});
