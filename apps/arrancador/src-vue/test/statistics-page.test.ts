import { mount } from "@vue/test-utils";
import StatisticsPage from "@vue-app/pages/StatisticsPage.vue";
import type { PlaytimeStats } from "@/types";

const { statsApiMock } = vi.hoisted(() => ({
  statsApiMock: {
    getPlaytimeStats: vi.fn(),
  },
}));

vi.mock("@/lib/api", () => ({
  statsApi: statsApiMock,
}));

const sampleStats: PlaytimeStats = {
  range_start: "2026-01-01",
  range_end: "2026-01-31",
  total_seconds: 3660,
  daily_totals: [
    { date: "2026-01-01", seconds: 0 },
    { date: "2026-01-02", seconds: 60 },
    { date: "2026-01-03", seconds: 3600 },
  ],
  per_game_totals: Array.from({ length: 9 }).map((_, index) => ({
    id: `g${index}`,
    name:
      index === 0
        ? "A very very long game title that must be truncated"
        : `Game ${index}`,
    seconds: (index + 1) * 300,
  })),
};

async function flushAsyncWork() {
  await Promise.resolve();
  await Promise.resolve();
}

describe("StatisticsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-04-20T12:00:00"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders stats and reloads when preset/month/custom ranges change", async () => {
    statsApiMock.getPlaytimeStats.mockResolvedValue(sampleStats);

    const wrapper = mount(StatisticsPage);
    await flushAsyncWork();

    expect(wrapper.text()).toContain("Статистика");
    expect(wrapper.text()).toContain("Топ:");
    expect(statsApiMock.getPlaytimeStats).toHaveBeenCalledWith("2026-03-22", "2026-04-20");

    const presetButton = wrapper
      .findAll("button")
      .find((candidate) => candidate.text() === "7 дней");
    expect(presetButton).toBeDefined();
    await presetButton!.trigger("click");
    await flushAsyncWork();

    expect(statsApiMock.getPlaytimeStats).toHaveBeenLastCalledWith("2026-04-14", "2026-04-20");

    await wrapper.get("#stats-month").setValue("2026-03");
    await flushAsyncWork();
    expect(statsApiMock.getPlaytimeStats).toHaveBeenLastCalledWith("2026-03-01", "2026-03-31");

    await wrapper.get("#stats-start-date").setValue("2026-04-05");
    await flushAsyncWork();
    expect(statsApiMock.getPlaytimeStats).toHaveBeenLastCalledWith("2026-04-05", "2026-04-05");

    await wrapper.get("#stats-end-date").setValue("2026-04-18");
    await flushAsyncWork();
    expect(statsApiMock.getPlaytimeStats).toHaveBeenLastCalledWith("2026-04-05", "2026-04-18");
  });

  it("shows the initial error state when stats cannot be loaded", async () => {
    statsApiMock.getPlaytimeStats.mockRejectedValueOnce(new Error("boom"));
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const wrapper = mount(StatisticsPage);
    await flushAsyncWork();

    expect(wrapper.text()).toContain("Не удалось загрузить статистику");

    errorSpy.mockRestore();
  });
});
