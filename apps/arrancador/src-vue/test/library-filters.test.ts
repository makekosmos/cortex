import { describe, expect, it } from "vitest";
import type { Game } from "../../src/types";
import {
  countActiveLibraryFilters,
  createDefaultLibraryFilterPreset,
  filterLibraryGames,
  formatPlaytime,
  getLibraryMetadataOptions,
  hasActiveLibraryFilters,
  sanitizeLibraryFilterPreset,
} from "../lib/libraryFilters";

const makeGame = (overrides: Partial<Game>): Game =>
  ({
    id: "game-1",
    ark_object_id: null,
    name: "Alpha",
    exe_path: "C:\\Games\\Alpha\\alpha.exe",
    exe_name: "alpha.exe",
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
    date_added: "2026-04-20T00:00:00.000Z",
    backup_enabled: false,
    last_backup: null,
    backup_count: 0,
    save_path: null,
    user_rating: null,
    user_note: null,
    ...overrides,
  }) satisfies Game;

describe("library filters", () => {
  it("sanitizes stored presets and ignores invalid enum values", () => {
    const preset = sanitizeLibraryFilterPreset({
      viewMode: "table",
      sortBy: "playtime",
      selectedGenres: ["RPG", 42],
      installState: "missing",
      showFavoritesOnly: true,
    });

    expect(preset.viewMode).toBe("grid");
    expect(preset.sortBy).toBe("playtime");
    expect(preset.selectedGenres).toEqual(["RPG"]);
    expect(preset.installState).toBe("all");
    expect(preset.showFavoritesOnly).toBe(true);
  });

  it("counts active advanced filters consistently", () => {
    const filters = {
      ...createDefaultLibraryFilterPreset(),
      showFavoritesOnly: true,
      selectedGenres: ["RPG", "Action"],
      minPlaytimeHours: "2,5",
      requireMetadata: true,
    };

    expect(hasActiveLibraryFilters(filters)).toBe(true);
    expect(countActiveLibraryFilters(filters)).toBe(5);
  });

  it("builds sorted metadata options from comma-separated fields", () => {
    expect(
      getLibraryMetadataOptions(
        [
          makeGame({ genres: "RPG, Action" }),
          makeGame({ genres: "Strategy, RPG" }),
        ],
        "genres",
      ),
    ).toEqual(["Action", "RPG", "Strategy"]);
  });

  it("filters by search, favorite, metadata, install state, and play status", () => {
    const games = [
      makeGame({
        id: "alpha",
        name: "Alpha Quest",
        is_favorite: true,
        rawg_id: 1,
        genres: "RPG",
        platforms: "PC",
        play_status: "in_progress",
        total_playtime: 7200,
      }),
      makeGame({
        id: "beta",
        name: "Beta Arena",
        is_favorite: true,
        genres: "Action",
        platforms: "Console",
        play_status: "completed",
      }),
      makeGame({
        id: "gamma",
        name: "Gamma",
        is_favorite: false,
        rawg_id: 3,
        genres: "RPG",
        platforms: "PC",
        play_status: "in_progress",
      }),
    ];

    const result = filterLibraryGames(
      games,
      {
        ...createDefaultLibraryFilterPreset(),
        searchQuery: "quest",
        showFavoritesOnly: true,
        selectedGenres: ["RPG"],
        selectedPlatforms: ["PC"],
        installState: "installed",
        playStatusState: "in_progress",
        requireMetadata: true,
      },
      { alpha: true, beta: false, gamma: true },
    );

    expect(result.map((game) => game.id)).toEqual(["alpha"]);
  });

  it("sorts by playtime descending", () => {
    const result = filterLibraryGames(
      [
        makeGame({ id: "short", name: "Short", total_playtime: 60 }),
        makeGame({ id: "long", name: "Long", total_playtime: 3600 }),
      ],
      {
        ...createDefaultLibraryFilterPreset(),
        sortBy: "playtime",
      },
      {},
    );

    expect(result.map((game) => game.id)).toEqual(["long", "short"]);
  });

  it("formats compact playtime labels", () => {
    expect(formatPlaytime(0)).toBe("0 ч");
    expect(formatPlaytime(90)).toBe("1 мин");
    expect(formatPlaytime(3900)).toBe("1 ч 5 мин");
  });
});
