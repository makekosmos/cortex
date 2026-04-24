import { describe, expect, it } from "vitest";
import type { Game } from "../../src/types";
import {
  cleanNameFromFile,
  fileNameFromPath,
  findMergeCandidate,
  isSupportedDropPath,
  matchMergeNames,
  normalizeNameForMerge,
} from "../lib/gameImport";

const makeGame = (overrides: Partial<Game>): Game =>
  ({
    id: "game-1",
    ark_object_id: null,
    name: "Elden Ring",
    exe_path: "C:\\Games\\Elden Ring\\eldenring.exe",
    exe_name: "eldenring.exe",
    process_bindings: [],
    play_status: "not_started",
    rawg_id: null,
    description: null,
    released: null,
    background_image: null,
    metacritic: null,
    rating: null,
    genres: null,
    platforms: null,
    developers: null,
    publishers: null,
    cover_image: null,
    icon_image: null,
    is_favorite: false,
    play_count: 0,
    total_playtime: 0,
    last_played: null,
    date_added: "2026-04-23T00:00:00.000Z",
    backup_enabled: false,
    last_backup: null,
    backup_count: 0,
    save_path: null,
    user_rating: null,
    user_note: null,
    ...overrides,
  }) satisfies Game;

describe("game import helpers", () => {
  it("accepts executable and shortcut paths only", () => {
    expect(isSupportedDropPath("C:\\Games\\Game.exe")).toBe(true);
    expect(isSupportedDropPath("C:\\Games\\Game.LNK")).toBe(true);
    expect(isSupportedDropPath("C:\\Games\\readme.txt")).toBe(false);
  });

  it("extracts file names from Windows and POSIX paths", () => {
    expect(fileNameFromPath("C:\\Games\\Elden Ring\\eldenring.exe")).toBe(
      "eldenring.exe",
    );
    expect(fileNameFromPath("/games/Hades/Hades.exe")).toBe("Hades.exe");
  });

  it("derives readable names from executable and shortcut filenames", () => {
    expect(cleanNameFromFile("elden_ring.exe")).toBe("elden ring");
    expect(cleanNameFromFile("Cyberpunk-2077.lnk")).toBe("Cyberpunk 2077");
  });

  it("normalizes noisy names before merge matching", () => {
    expect(normalizeNameForMerge("Elden Ring [Steam].exe")).toBe("elden ring");
    expect(matchMergeNames("the witcher 3 wild hunt", "witcher 3 wild hunt")).toBe(
      true,
    );
  });

  it("finds merge candidates by display name, executable name, or path filename", () => {
    const games = [
      makeGame({
        id: "elden",
        name: "Elden Ring",
        exe_path: "C:\\Games\\Elden Ring\\start.exe",
        exe_name: "start.exe",
      }),
      makeGame({
        id: "hades",
        name: "Hades",
        exe_path: "D:\\Library\\Supergiant\\Hades.exe",
        exe_name: "Hades.exe",
      }),
    ];

    expect(findMergeCandidate(games, "elden_ring.exe")?.id).toBe("elden");
    expect(findMergeCandidate(games, "Hades.lnk")?.id).toBe("hades");
  });
});
