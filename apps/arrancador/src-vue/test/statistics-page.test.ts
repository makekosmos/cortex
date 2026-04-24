import { flushPromises, mount } from "@vue/test-utils";
import { addDays, HEATMAP_RANGE_DAYS, toIsoDate } from "@vue-app/lib/statistics";
import StatisticsPage from "@vue-app/pages/StatisticsPage.vue";
import { nextTick } from "vue";
import type { PlaytimeStats } from "@/types";

const { statsApiMock } = vi.hoisted(() => ({
  statsApiMock: {
    getPlaytimeStats: vi.fn(),
  },
}));

vi.mock("@/lib/api", () => ({
  statsApi: statsApiMock,
}));

const rangeStats: PlaytimeStats = {
  range_start: "2025-04-22",
  range_end: "2026-04-21",
  total_seconds: 16200,
  daily_totals: [
    { date: "2026-04-19", seconds: 7200 },
    { date: "2026-04-20", seconds: 1800 },
    { date: "2026-04-21", seconds: 7200 },
  ],
  per_game_totals: [
    { id: "valorant", name: "Valorant", seconds: 8400 },
    { id: "doom", name: "DOOM", seconds: 7800 },
  ],
};

const todayStats: PlaytimeStats = {
  range_start: "2026-04-21",
  range_end: "2026-04-21",
  total_seconds: 7200,
  daily_totals: [{ date: "2026-04-21", seconds: 7200 }],
  per_game_totals: [{ id: "valorant", name: "Valorant", seconds: 7200 }],
};

const yesterdayStats: PlaytimeStats = {
  range_start: "2026-04-20",
  range_end: "2026-04-20",
  total_seconds: 1800,
  daily_totals: [{ date: "2026-04-20", seconds: 1800 }],
  per_game_totals: [{ id: "doom", name: "DOOM", seconds: 1800 }],
};

async function flushAsyncWork(wrapper?: { vm: { $forceUpdate: () => void; $nextTick: () => Promise<void> } }) {
  for (let index = 0; index < 5; index += 1) {
    await flushPromises();
    await nextTick();
    await vi.runAllTimersAsync();
  }
  await Promise.allSettled(
    statsApiMock.getPlaytimeStats.mock.results
      .map((result) => result.value)
      .filter((value): value is Promise<PlaytimeStats> => Boolean(value)),
  );
  await flushPromises();
  await nextTick();
  wrapper?.vm.$forceUpdate();
  await wrapper?.vm.$nextTick();
}

describe("StatisticsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-04-21T12:00:00"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the heatmap, selects today by default, and reloads selected-day stats", async () => {
    const today = new Date("2026-04-21T12:00:00");
    const expectedStart = toIsoDate(addDays(today, -(HEATMAP_RANGE_DAYS - 1)));
    const expectedEnd = "2026-04-21";

    statsApiMock.getPlaytimeStats.mockImplementation(async (start?: string, end?: string) => {
      if (start === expectedStart && end === expectedEnd) {
        return rangeStats;
      }
      if (start === "2026-04-21" && end === "2026-04-21") {
        return todayStats;
      }
      if (start === "2026-04-20" && end === "2026-04-20") {
        return yesterdayStats;
      }
      throw new Error(`Unexpected range ${String(start)}..${String(end)}`);
    });

    const wrapper = mount(StatisticsPage);
    await flushAsyncWork(wrapper);
    expect(wrapper.text()).toContain("Статистика");
    expect(wrapper.text()).toContain("вторник, 21 апреля");
    expect(wrapper.text()).toContain("2.0 ч");
    expect(statsApiMock.getPlaytimeStats).toHaveBeenNthCalledWith(1, expectedStart, expectedEnd);
    expect(statsApiMock.getPlaytimeStats).toHaveBeenNthCalledWith(2, "2026-04-21", "2026-04-21");

    await wrapper.get('[data-date="2026-04-20"]').trigger("click");
    await flushAsyncWork(wrapper);

    expect(wrapper.text()).toContain("понедельник, 20 апреля");
    expect(wrapper.text()).toContain("DOOM");
    expect(statsApiMock.getPlaytimeStats).toHaveBeenLastCalledWith("2026-04-20", "2026-04-20");
  });

  it("shows the initial error state when the heatmap cannot be loaded", async () => {
    statsApiMock.getPlaytimeStats.mockRejectedValueOnce(new Error("boom"));
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const wrapper = mount(StatisticsPage);
    await flushAsyncWork(wrapper);

    expect(wrapper.text()).toContain("Не удалось загрузить тепловую карту статистики");

    errorSpy.mockRestore();
  });
});
