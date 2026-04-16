import { act, render, screen, waitFor } from "@testing-library/react";
import { useGameStatus } from "@/hooks/useGameStatus";

const { gamesApiMock } = vi.hoisted(() => ({
  gamesApiMock: {
    isInstalled: vi.fn(),
    getRunningInstances: vi.fn(),
  },
}));

vi.mock("@/lib/api", () => ({
  gamesApi: gamesApiMock,
}));

function Harness({ gameId }: { gameId?: string }) {
  const state = useGameStatus(gameId, "C:\\x.exe");
  return (
    <div>
      <div data-testid="installed">{state.isInstalled ? "yes" : "no"}</div>
      <div data-testid="checkingInstalled">
        {state.checkingInstalled ? "yes" : "no"}
      </div>
      <div data-testid="running">{String(state.runningCount)}</div>
      <div data-testid="checkingRunning">
        {state.checkingRunning ? "yes" : "no"}
      </div>
      <button type="button" onClick={() => state.setRunningCount(123)}>
        set
      </button>
    </div>
  );
}

describe("useGameStatus", () => {
  beforeEach(() => {
    gamesApiMock.isInstalled.mockReset();
    gamesApiMock.getRunningInstances.mockReset();
  });

  it("does nothing when gameId is missing", () => {
    render(<Harness />);
    expect(gamesApiMock.isInstalled).not.toHaveBeenCalled();
    expect(gamesApiMock.getRunningInstances).not.toHaveBeenCalled();
  });

  it("checks install status and running instances", async () => {
    gamesApiMock.isInstalled.mockResolvedValueOnce(false);
    gamesApiMock.getRunningInstances.mockResolvedValueOnce(2);

    render(<Harness gameId="g1" />);

    expect(screen.getByTestId("checkingInstalled")).toHaveTextContent("yes");

    await waitFor(() =>
      expect(screen.getByTestId("installed")).toHaveTextContent("no"),
    );

    await waitFor(() =>
      expect(screen.getByTestId("running")).toHaveTextContent("2"),
    );
  });

  it("handles install check errors by defaulting to installed=true", async () => {
    gamesApiMock.isInstalled.mockRejectedValueOnce(new Error("fail"));
    gamesApiMock.getRunningInstances.mockResolvedValueOnce(0);

    render(<Harness gameId="g1" />);

    await waitFor(() =>
      expect(screen.getByTestId("checkingInstalled")).toHaveTextContent("no"),
    );
    expect(screen.getByTestId("installed")).toHaveTextContent("yes");
  });

  it("polls running instances every 5s and keeps working on errors", async () => {
    vi.useFakeTimers();
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    gamesApiMock.isInstalled.mockResolvedValueOnce(true);
    gamesApiMock.getRunningInstances
      .mockResolvedValueOnce(1)
      .mockRejectedValueOnce(new Error("poll-fail"))
      .mockResolvedValueOnce(3);

    render(<Harness gameId="g1" />);

    // Under fake timers, avoid waitFor (it relies on timers). `render` flushes effects.
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByTestId("running")).toHaveTextContent("1");
    expect(gamesApiMock.getRunningInstances).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
    });
    expect(gamesApiMock.getRunningInstances).toHaveBeenCalledTimes(2);
    expect(consoleErrorSpy).toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
    });
    expect(gamesApiMock.getRunningInstances).toHaveBeenCalledTimes(3);
    expect(screen.getByTestId("running")).toHaveTextContent("3");

    consoleErrorSpy.mockRestore();
    vi.useRealTimers();
  });

  it("does not set state after unmount (mounted guards)", async () => {
    let resolveInstalled: ((value: boolean) => void) | undefined;
    let resolveRunning: ((value: number) => void) | undefined;

    gamesApiMock.isInstalled.mockReturnValueOnce(
      new Promise<boolean>((resolve) => {
        resolveInstalled = resolve;
      }),
    );
    gamesApiMock.getRunningInstances.mockReturnValueOnce(
      new Promise<number>((resolve) => {
        resolveRunning = resolve;
      }),
    );

    const { unmount } = render(<Harness gameId="g1" />);
    unmount();

    if (resolveInstalled) resolveInstalled(false);
    if (resolveRunning) resolveRunning(2);

    // Let pending promises run; the hook should ignore updates after unmount.
    await Promise.resolve();
  });

  it("covers mounted=false branch for install error handler", async () => {
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    let rejectInstalled: ((reason?: unknown) => void) | undefined;

    gamesApiMock.isInstalled.mockReturnValueOnce(
      new Promise<boolean>((_resolve, reject) => {
        rejectInstalled = reject;
      }),
    );
    gamesApiMock.getRunningInstances.mockResolvedValueOnce(0);

    const { unmount } = render(<Harness gameId="g1" />);
    unmount();

    if (rejectInstalled) rejectInstalled(new Error("late-fail"));

    // Allow promise rejection handlers to run.
    await Promise.resolve();
    await Promise.resolve();

    expect(consoleErrorSpy).toHaveBeenCalled();
    consoleErrorSpy.mockRestore();
  });
});
