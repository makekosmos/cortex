import { describe, expect, test } from "bun:test";
import { addLocalDays, breakdown, localDayKey, streaks } from "./useCoderStats";

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
});
