import { describe, expect, it } from "vitest";
import { mapGameRow } from "./rows";

describe("game row mapping", () => {
  it("coerces database row values into a stable game snapshot", () => {
    const game = mapGameRow({
      id: "game-1",
      ark_object_id: null,
      name: "Control",
      exe_path: "C:\\Games\\Control\\Control.exe",
      exe_name: "Control.exe",
      rawg_id: "42",
      description: "A game",
      released: null,
      background_image: null,
      metacritic: "85",
      rating: "4.5",
      genres: "Action",
      platforms: null,
      developers: null,
      publishers: null,
      cover_image: null,
      icon_image: null,
      is_favorite: 1,
      play_count: "3",
      total_playtime: "120",
      last_played: "2026-04-24T00:00:00.000Z",
      date_added: "2026-04-20T00:00:00.000Z",
      backup_enabled: 0,
      last_backup: null,
      backup_count: "2",
      save_path: null,
      user_rating: "9",
      user_note: null,
      play_status: null,
    });

    expect(game).toMatchObject({
      id: "game-1",
      process_bindings: [],
      play_status: "not_started",
      rawg_id: 42,
      metacritic: 85,
      rating: 4.5,
      is_favorite: true,
      backup_enabled: false,
      play_count: 3,
      total_playtime: 120,
      backup_count: 2,
      user_rating: 9,
    });
  });
});
