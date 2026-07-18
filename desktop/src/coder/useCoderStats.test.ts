import { describe, expect, test } from "bun:test";
import {
  addLocalDays,
  breakdown,
  codewarsProfileStats,
  localDayKey,
  profileDifficultyStats,
  refreshCoderStats,
  streaks,
} from "./useCoderStats";

describe("coder statistics", () => {
  test("groups values by count and calculates shares", () => {
    expect(breakdown(["Rust", "TypeScript", "Rust"], 3)).toEqual([
      { label: "Rust", count: 2, share: 2 / 3 },
      { label: "TypeScript", count: 1, share: 1 / 3 },
    ]);
  });

  test("counts a current streak that ends today or yesterday", () => {
    const today = new Date();
    const days = new Set([
      localDayKey(addLocalDays(today, -3)),
      localDayKey(addLocalDays(today, -2)),
      localDayKey(addLocalDays(today, -1)),
    ]);
    expect(streaks(days)).toEqual({ current: 3, longest: 3 });
  });

  test("reads exact LeetCode profile counts", () => {
    expect(
      profileDifficultyStats([
        {
          id: "profile",
          propsJson: {
            source: "leetcode",
            solved: { all: 78, easy: 61, medium: 17, hard: 0 },
            available: { all: 3991, easy: 954, medium: 2084, hard: 953 },
          },
        },
      ]),
    ).toEqual({
      easy: 61,
      easyTotal: 954,
      medium: 17,
      mediumTotal: 2084,
      hard: 0,
      hardTotal: 953,
      total: 78,
      available: 3991,
    });
  });

  test("refresh synchronizes LeetCode before reading local stats", async () => {
    // Regression: 2026-07-18. The Coder refresh button only reread ARK, so a missing
    // coding_profile_obj stayed missing forever.
    const calls: string[] = [];
    await refreshCoderStats(
      async (operation, params) => {
        calls.push(`${operation}:${String(params.provider)}`);
      },
      "leetcode",
      async () => {
        calls.push("load");
      },
    );
    expect(calls).toEqual(["integrations.sync_now:leetcode", "load"]);
  });

  test("reads Codewars public profile metrics", () => {
    expect(
      codewarsProfileStats([
        {
          id: "profile",
          propsJson: {
            source: "codewars",
            username: "tester",
            honor: 544,
            leaderboardPosition: 134,
            rank: { name: "3 kyu", score: 2116 },
          },
        },
      ]),
    ).toEqual({
      username: "tester",
      honor: 544,
      leaderboardPosition: 134,
      rankName: "3 kyu",
      rankScore: 2116,
    });
  });
});
