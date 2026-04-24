import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import type { DbLike } from "../../helpers/shared";
import { buildUpdateClause } from "./update";

function createDb(get: DbLike["get"]): DbLike {
  return {
    all: async () => [],
    get,
    run: async () => ({ changes: 0 }),
  };
}

describe("game update clause builder", () => {
  it("rejects empty names before building SQL", async () => {
    await expect(
      buildUpdateClause(
        { id: "game-1", name: "   " },
        createDb(async () => undefined),
      ),
    ).rejects.toThrow("name cannot be empty");
  });

  it("updates executable path and derived executable name together", async () => {
    const clause = await buildUpdateClause(
      { id: "game-1", exe_path: "C:\\Games\\Control\\Control.exe" },
      createDb(async () => undefined),
    );

    expect(clause.sql).toBe("UPDATE games SET exe_path = ?, exe_name = ? WHERE id = ?");
    expect(clause.values).toEqual([
      "C:\\Games\\Control\\Control.exe",
      "Control.exe",
      "game-1",
    ]);
  });

  it("rejects executable paths already attached to another game binding", async () => {
    await expect(
      buildUpdateClause(
        { id: "game-1", exe_path: "C:\\Games\\Other\\Other.exe" },
        createDb(async () => ({ game_id: "game-2" })),
      ),
    ).rejects.toThrow("exe_path is already attached to another game");
  });

  it("normalizes save paths inside the game directory to the game path token", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "arrancador-update-"));
    const gameDir = path.join(root, "Game");
    const saveDir = path.join(gameDir, "Saves");
    await mkdir(saveDir, { recursive: true });
    await writeFile(path.join(gameDir, "Game.exe"), "");

    const clause = await buildUpdateClause(
      { id: "game-1", save_path: saveDir },
      createDb(async () => ({ exe_path: path.join(gameDir, "Game.exe") })),
    );

    expect(clause.values).toEqual(["{PATHTOGAME}\\Saves", 1, "game-1"]);
  });
});
