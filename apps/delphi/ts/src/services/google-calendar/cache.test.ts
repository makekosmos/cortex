import {
  expandRequestedRange,
  isRangeCovered,
  shouldRefreshCoverage,
} from "@/services/google-calendar/cache";

describe("google calendar cache coverage", () => {
  it("detects when a requested range is already covered", () => {
    expect(
      isRangeCovered(
        "2026-04-01T00:00:00.000Z",
        "2026-05-01T00:00:00.000Z",
        "2026-04-10T00:00:00.000Z",
        "2026-04-20T00:00:00.000Z",
      ),
    ).toBe(true);
  });

  it("expands requests to add padding around the visible range", () => {
    expect(
      expandRequestedRange(
        "2026-04-10T00:00:00.000Z",
        "2026-04-20T00:00:00.000Z",
        7,
      ),
    ).toEqual({
      start: "2026-04-03T00:00:00.000Z",
      end: "2026-04-27T00:00:00.000Z",
    });
  });

  it("requests refresh when cache is stale even if range is covered", () => {
    expect(
      shouldRefreshCoverage({
        coverageStart: "2026-04-01T00:00:00.000Z",
        coverageEnd: "2026-05-01T00:00:00.000Z",
        requestedStart: "2026-04-10T00:00:00.000Z",
        requestedEnd: "2026-04-20T00:00:00.000Z",
        lastSyncAt: "2026-04-10T08:00:00.000Z",
        now: Date.parse("2026-04-10T08:16:00.000Z"),
        staleAfterMs: 15 * 60 * 1000,
      }),
    ).toBe(true);
  });
});
