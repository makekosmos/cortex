import { describe, expect, it } from "vitest";
import {
  buildLegacyUsageBackfillPlan,
  type LegacyDailyRow,
  type LegacyGameRow,
} from "./plan";

const baseGame = {
  id: "game-1",
  name: "Alpha",
  exe_path: "C:\\Games\\Alpha\\alpha.exe",
  exe_name: "alpha.exe",
  date_added: "2026-02-03T10:00:00.000Z",
  total_playtime: 0,
  last_played: null,
} satisfies LegacyGameRow;

describe("legacy usage backfill plan", () => {
  it("skips games when legacy totals and daily rows contain no usable playtime", () => {
    const plan = buildLegacyUsageBackfillPlan(
      [{ ...baseGame, total_playtime: -5 }],
      [{ game_id: "game-1", date: "2026-02-03", seconds: -60 } satisfies LegacyDailyRow],
      "device-1",
      "Test Device",
    );

    expect(plan.trackedApps).toEqual([]);
    expect(plan.sessions).toEqual([]);
  });

  it("creates residual sessions from date_added when no better legacy date exists", () => {
    const plan = buildLegacyUsageBackfillPlan(
      [{ ...baseGame, total_playtime: 120 }],
      [],
      "device-1",
      "Test Device",
    );

    expect(plan.trackedApps).toHaveLength(1);
    expect(plan.sessions).toHaveLength(1);
    expect(plan.sessions[0]).toMatchObject({
      kind: "residual",
      foregroundMs: 120000,
      endedAt: "2026-02-03T21:00:00.000Z",
      windowTitle: "Alpha",
    });
  });
});
