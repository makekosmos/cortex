import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import AchievementsPage from "@/pages/Achievements";
import type { Achievement } from "@/types";

const { notifyMock, achievementsApiMock } = vi.hoisted(() => ({
  notifyMock: vi.fn(),
  achievementsApiMock: {
    getAll: vi.fn(),
    seedDefaults: vi.fn(),
    recordEvent: vi.fn(),
  },
}));

vi.mock("@/components/ToastProvider", () => ({
  useToast: () => ({ notify: notifyMock }),
}));
vi.mock("@/lib/api", () => ({
  achievementsApi: achievementsApiMock,
}));

describe("AchievementsPage", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    notifyMock.mockReset();
    achievementsApiMock.getAll.mockReset();
    achievementsApiMock.seedDefaults.mockReset();
    achievementsApiMock.recordEvent.mockReset();
  });

  const unlocked: Achievement = {
    id: "ach-1",
    title: "First Launch",
    description: "Launch a game once",
    event_trigger: "game_launch",
    progress: 1,
    target: 1,
    unlocked: true,
    unlocked_at: "2024-01-02T00:00:00.000Z",
    created_at: "2024-01-01T00:00:00.000Z",
  };

  const locked: Achievement = {
    id: "ach-2",
    title: "Backup Keeper",
    description: "Create a backup",
    event_trigger: "backup_restore",
    progress: 0,
    target: 3,
    unlocked: false,
    unlocked_at: null,
    created_at: "2024-01-01T00:00:00.000Z",
  };

  it("loads achievements and switches filters", async () => {
    achievementsApiMock.getAll.mockImplementation((unlockedOnly: boolean) =>
      Promise.resolve(unlockedOnly ? [unlocked] : [unlocked, locked]),
    );

    render(<AchievementsPage />);

    expect(await screen.findByTestId("achievements-kpi")).toHaveTextContent(
      "Unlocked: 1 / 2",
    );
    expect(
      within(await screen.findByTestId("achievement-card-ach-1")).getByText(
        "First Launch",
      ),
    ).toBeInTheDocument();
    expect(
      within(await screen.findByTestId("achievement-card-ach-2")).getByText(
        "Backup Keeper",
      ),
    ).toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(screen.getByTestId("achievements-filter-unlocked"));
    await waitFor(() => expect(achievementsApiMock.getAll).toHaveBeenLastCalledWith(true));
    expect(
      within(await screen.findByTestId("achievement-card-ach-1")).getByText(
        "First Launch",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("achievement-card-ach-2")).not.toBeInTheDocument();

    await user.click(screen.getByTestId("achievements-filter-locked"));
    await waitFor(() => expect(achievementsApiMock.getAll).toHaveBeenLastCalledWith(false));
    expect(
      within(await screen.findByTestId("achievement-card-ach-2")).getByText(
        "Backup Keeper",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("achievement-card-ach-1")).not.toBeInTheDocument();
  });

  it("supports seed, record, export, and import actions", async () => {
    achievementsApiMock.getAll.mockResolvedValue([unlocked, locked]);
    achievementsApiMock.seedDefaults.mockResolvedValue([unlocked, locked]);
    achievementsApiMock.recordEvent.mockResolvedValue([unlocked, locked]);

    const promptSpy = vi.spyOn(window, "prompt");
    const writeText = vi
      .spyOn(navigator.clipboard, "writeText")
      .mockResolvedValue(undefined);

    render(<AchievementsPage />);
    expect(await screen.findByTestId("achievement-card-ach-1")).toBeInTheDocument();

    const user = userEvent.setup();
    await user.type(
      screen.getByTestId("achievements-context-input"),
      " Arcadia ",
    );
    await user.click(screen.getByTestId("achievements-record"));

    await waitFor(() =>
      expect(achievementsApiMock.recordEvent).toHaveBeenCalledWith(
        "game_launch",
        "Arcadia",
      ),
    );

    await user.click(screen.getByTestId("achievements-seed"));
    await waitFor(() => expect(achievementsApiMock.seedDefaults).toHaveBeenCalled());

    await user.click(screen.getByTestId("achievements-export"));
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("First Launch"));

    promptSpy.mockReturnValueOnce(
      JSON.stringify([
        {
          id: "imported",
          title: "Imported",
          description: "Imported achievement",
          event_trigger: "scan_complete",
          progress: 1,
          target: 5,
          unlocked: false,
          unlocked_at: null,
          created_at: "2024-01-03T00:00:00.000Z",
        },
      ]),
    );
    await user.click(screen.getByTestId("achievements-import"));
    expect(await screen.findByTestId("achievement-card-imported")).toBeInTheDocument();

    promptSpy.mockReturnValueOnce("not json");
    await user.click(screen.getByTestId("achievements-import"));
    await waitFor(() =>
      expect(notifyMock).toHaveBeenCalledWith(
        expect.objectContaining({ tone: "error" }),
      ),
    );

    promptSpy.mockRestore();
  });
});
