import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import Statistics from "@/pages/Statistics";
import type { PlaytimeStats } from "@/types";

const { statsApiMock } = vi.hoisted(() => ({
  statsApiMock: {
    getPlaytimeStats: vi.fn(),
  },
}));

vi.mock("recharts", async () => {
  const passthrough = ({
    children,
  }: {
    children?: import("react").ReactNode;
  }) => <div>{children}</div>;

  return {
    ResponsiveContainer: passthrough,
    AreaChart: passthrough,
    BarChart: passthrough,
    CartesianGrid: () => null,
    Area: () => null,
    Bar: () => null,
    XAxis: (props: { tickFormatter?: (value: unknown) => unknown }) => {
      props.tickFormatter?.("2026-01-01");
      return null;
    },
    YAxis: (props: { tickFormatter?: (value: unknown) => unknown }) => {
      props.tickFormatter?.(1);
      return null;
    },
    Tooltip: (props: {
      formatter?: (value: unknown, name: unknown, item: unknown) => unknown;
      labelFormatter?: (label: unknown) => unknown;
    }) => {
      props.labelFormatter?.("2026-01-01");
      props.formatter?.(0, "x", { payload: { seconds: 62 } });
      return null;
    },
  };
});

vi.mock("@/lib/api", () => ({
  statsApi: statsApiMock,
}));

describe("Statistics", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders stats and supports preset/month/custom range interactions", async () => {
    const stats: PlaytimeStats = {
      range_start: "2026-01-01",
      range_end: "2026-01-31",
      total_seconds: 3660,
      daily_totals: [
        { date: "2026-01-01", seconds: 0 },
        { date: "2026-01-02", seconds: 60 },
      ],
      per_game_totals: Array.from({ length: 9 }).map((_, i) => ({
        id: `g${i}`,
        name:
          i === 0
            ? "A very very long game title that must be truncated"
            : `Game ${i}`,
        seconds: i * 60,
      })),
    };

    statsApiMock.getPlaytimeStats.mockResolvedValue(stats);

    render(<Statistics />);

    expect(await screen.findByText(/статистика/i)).toBeInTheDocument();
    await waitFor(() =>
      expect(statsApiMock.getPlaytimeStats).toHaveBeenCalledTimes(1),
    );

    await userEvent.click(screen.getByRole("button", { name: /^7/i }));
    await waitFor(() =>
      expect(statsApiMock.getPlaytimeStats).toHaveBeenCalledTimes(2),
    );

    const monthTrigger = document.getElementById("stats-month");
    const startTrigger = document.getElementById("stats-start-date");
    const endTrigger = document.getElementById("stats-end-date");
    if (!monthTrigger || !startTrigger || !endTrigger) {
      throw new Error("missing range triggers");
    }

    await userEvent.click(monthTrigger);
    await userEvent.click((await screen.findAllByRole("menuitem"))[0]!);
    await waitFor(() =>
      expect(statsApiMock.getPlaytimeStats).toHaveBeenCalledTimes(3),
    );

    await userEvent.click(startTrigger);
    const startOptions = await screen.findAllByRole("menuitem");
    await userEvent.click(startOptions[1] ?? startOptions[0]!);
    await waitFor(() =>
      expect(statsApiMock.getPlaytimeStats).toHaveBeenCalledTimes(4),
    );

    await userEvent.click(endTrigger);
    const endOptions = await screen.findAllByRole("menuitem");
    await userEvent.click(endOptions[2] ?? endOptions[0]!);
    await waitFor(() =>
      expect(statsApiMock.getPlaytimeStats).toHaveBeenCalledTimes(5),
    );
  }, 10000);

  it("shows error state when stats cannot be loaded", async () => {
    statsApiMock.getPlaytimeStats.mockRejectedValueOnce(new Error("boom"));
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<Statistics />);

    expect(await screen.findByText(/загрузить статистику/i)).toBeInTheDocument();

    errorSpy.mockRestore();
  });
});
