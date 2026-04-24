import { describe, expect, it } from "vitest";
import type { Game } from "../../src/types";
import {
  formatBytes,
  formatPlayedHours,
  formatPlaytime,
  normalizeDescription,
  PLAY_STATUS_LABELS,
  resolveSavePathTemplate,
} from "../lib/gameDetailDisplay";

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

describe("game detail display helpers", () => {
  it("formats playtime and played-hours labels", () => {
    expect(formatPlaytime(0)).toBe("0 ч");
    expect(formatPlaytime(125)).toBe("2 мин");
    expect(formatPlaytime(3660)).toBe("1 ч 1 мин");
    expect(formatPlayedHours(0)).toBe("0 ч");
    expect(formatPlayedHours(900)).toBe("<1 ч");
    expect(formatPlayedHours(7200)).toBe("2 ч");
  });

  it("formats byte sizes", () => {
    expect(formatBytes(0)).toBe("0 Б");
    expect(formatBytes(512)).toBe("512 Б");
    expect(formatBytes(1536)).toBe("1.5 КБ");
    expect(formatBytes(10 * 1024 * 1024)).toBe("10 МБ");
  });

  it("normalizes HTML descriptions to plain compact text", () => {
    expect(normalizeDescription("<p>Hello <strong>world</strong></p>")).toBe(
      "Hello world",
    );
    expect(normalizeDescription("   ")).toBeNull();
  });

  it("resolves save-path templates relative to the game executable", () => {
    expect(
      resolveSavePathTemplate(
        "{PATHTOGAME}\\Saves",
        makeGame({ exe_path: "C:\\Games\\Alpha\\bin\\alpha.exe" }),
      ),
    ).toBe("C:\\Games\\Alpha\\bin\\Saves");
  });

  it("keeps user-facing play-status labels centralized", () => {
    expect(PLAY_STATUS_LABELS.completed).toBe("Пройдено");
  });
});
