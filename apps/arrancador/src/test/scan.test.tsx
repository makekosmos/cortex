import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import Scan from "@/pages/Scan";
import { useGamesActions, useGamesState } from "@/store/GamesContext";
import { arrancadorInvokeMock, arrancadorOnMock } from "./bridge";
import { createTestGame } from "@/types";

const { notifyMock, gamesApiMock, scanApiMock } = vi.hoisted(() => ({
  notifyMock: vi.fn(),
  gamesApiMock: {
    existsByPath: vi.fn(),
    resolveShortcutTarget: vi.fn(),
  },
  scanApiMock: {
    getRunningProcesses: vi.fn(),
  },
}));

vi.mock("@/store/GamesContext", () => ({
  useGamesActions: vi.fn(),
  useGamesState: vi.fn(),
}));
vi.mock("@/components/ToastProvider", () => ({
  useToast: () => ({ notify: notifyMock }),
}));
vi.mock("@/components/RawgMetadataPrompt", () => ({
  RawgMetadataPrompt: (props: {
    game: { name: string };
    onNext: () => void;
  }) => (
    <button type="button" onClick={props.onNext}>
      RAWG: {props.game.name}
    </button>
  ),
}));
vi.mock("@/lib/api", () => ({ gamesApi: gamesApiMock, scanApi: scanApiMock }));

const createDroppedFile = (path: string) => {
  const fileName = path.split(/[\\/]/).pop() ?? path;
  const file = new File([""], fileName, {
    type: "application/octet-stream",
  });
  Object.defineProperty(file, "path", {
    configurable: true,
    value: path,
  });
  return file;
};

const createDropData = (paths: string[]) => ({
  dataTransfer: {
    types: ["Files"],
    files: paths.map(createDroppedFile),
  },
});

const useGamesActionsMock = vi.mocked(useGamesActions);
const useGamesStateMock = vi.mocked(useGamesState);

describe("Scan", () => {
  const listeners = new Map<string, (payload: any) => void>();

  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();

    arrancadorOnMock.mockImplementation((eventName, cb) => {
      listeners.set(eventName as string, cb as (payload: any) => void);
      return () => {
        listeners.delete(eventName as string);
      };
    });

    arrancadorInvokeMock.mockImplementation(async (...args) => {
      const [channel] = args as [string, unknown?];

      if (channel === "dialog_open") {
        return null;
      }

      if (channel === "scan_executables_stream" || channel === "cancel_scan") {
        return undefined;
      }

      throw new Error(`Unexpected channel: ${String(channel)}`);
    });

    useGamesActionsMock.mockReturnValue({
      refreshGames: vi.fn(),
      addGame: vi.fn(),
      addGames: vi.fn().mockResolvedValue([]),
      updateGame: vi.fn(),
      deleteGame: vi.fn(),
      toggleFavorite: vi.fn(),
      searchGames: vi.fn(),
    });
    useGamesStateMock.mockReturnValue({
      games: [],
      favorites: [],
      loading: false,
      error: null,
      getGame: vi.fn(),
    });
  });

  it("loads running processes when switching tabs", async () => {
    scanApiMock.getRunningProcesses.mockResolvedValueOnce([
      {
        pid: 1,
        name: "SampleApp.exe",
        path: "C:\\Games\\SampleApp.exe",
        cpu_usage: 12,
        gpu_usage: 0,
      },
    ]);
    gamesApiMock.existsByPath.mockResolvedValue(false);

    render(<Scan />);

    const processesTab = screen.getByRole("button", { name: /Процессы/ });
    await userEvent.click(processesTab);

    await waitFor(() =>
      expect(scanApiMock.getRunningProcesses).toHaveBeenCalled(),
    );

    expect(await screen.findByDisplayValue("SampleApp")).toBeInTheDocument();
  });

  it("starts a folder scan and tolerates invoke failures", async () => {
    arrancadorInvokeMock
      .mockImplementationOnce(async () => null)
      .mockImplementationOnce(async (...args) => {
        const [channel] = args as [string];
        if (channel === "dialog_open") {
          return "C:\\Games";
        }
        return undefined;
      })
      .mockImplementationOnce(async (...args) => {
        const [channel] = args as [string];
        if (channel === "scan_executables_stream") {
          throw new Error("scan-fail");
        }
        return undefined;
      });

    render(<Scan />);

    const start = screen.getByTestId("scan-start");
    await userEvent.click(start);
    expect(arrancadorInvokeMock).toHaveBeenCalledWith(
      "dialog_open",
      expect.objectContaining({ directory: true }),
    );

    await userEvent.click(start);
    await waitFor(() =>
      expect(arrancadorInvokeMock).toHaveBeenCalledWith(
        "scan_executables_stream",
        {
          dir: "C:\\Games",
        },
      ),
    );
  });

  it("collects scan:entry events (including already-added) and stops scanning on scan:done", async () => {
    arrancadorInvokeMock.mockImplementation(async (...args) => {
      const [channel] = args as [string];
      if (channel === "dialog_open") {
        return "C:\\Games";
      }
      if (channel === "scan_executables_stream") {
        return undefined;
      }
      return undefined;
    });
    gamesApiMock.existsByPath
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true);

    render(<Scan />);

    await userEvent.click(screen.getByTestId("scan-start"));
    expect(await screen.findByText(/Сканирование/)).toBeInTheDocument();

    const onEntry = listeners.get("scan:entry");
    const onDone = listeners.get("scan:done");
    if (!onEntry || !onDone) throw new Error("missing scan listeners");

    await act(async () => {
      onEntry({
        path: "C:\\Games\\Arcadia.exe",
        file_name: "Arcadia.exe",
      });

      onEntry({
        path: "C:\\Games\\Existing.exe",
        file_name: "Existing.exe",
      });
    });

    expect(await screen.findByDisplayValue("Arcadia")).toBeInTheDocument();
    expect(await screen.findByText("Existing")).toBeInTheDocument();

    await act(async () => {
      onDone({ count: 2 });
    });
    await waitFor(() =>
      expect(screen.queryByText(/Сканирование/)).not.toBeInTheDocument(),
    );
  });

  it("handles dropped paths (success, warning, error)", async () => {
    const addGames = vi.fn().mockResolvedValueOnce([
      createTestGame({
        id: "g1",
        name: "Arcadia",
        exe_path: "C:\\Games\\Arcadia.exe",
        exe_name: "Arcadia.exe",
      }),
    ]);
    const refreshGames = vi.fn().mockResolvedValue(undefined);

    useGamesActionsMock.mockReturnValue({
      refreshGames,
      addGame: vi.fn(),
      addGames,
      updateGame: vi.fn(),
      deleteGame: vi.fn(),
      toggleFavorite: vi.fn(),
      searchGames: vi.fn(),
    });

    gamesApiMock.existsByPath.mockResolvedValue(false);

    const { container } = render(<Scan />);
    const dropTarget = container.firstElementChild as HTMLElement;
    if (!dropTarget) throw new Error("missing drop target");

    await act(async () => {
      fireEvent.drop(dropTarget, createDropData(["C:\\Games\\Arcadia.exe"]));
    });
    await waitFor(() =>
      expect(notifyMock).toHaveBeenCalledWith(
        expect.objectContaining({ tone: "success" }),
      ),
    );

    expect(await screen.findByText(/RAWG: Arcadia/)).toBeInTheDocument();
    await userEvent.click(screen.getByText(/RAWG: Arcadia/));

    notifyMock.mockClear();
    await act(async () => {
      fireEvent.drop(dropTarget, createDropData(["C:\\tmp\\note.txt"]));
    });
    await waitFor(() =>
      expect(notifyMock).toHaveBeenCalledWith(
        expect.objectContaining({ tone: "warning" }),
      ),
    );

    notifyMock.mockClear();
    addGames.mockRejectedValueOnce(new Error("nope"));
    await act(async () => {
      fireEvent.drop(dropTarget, createDropData(["C:\\Games\\Arcadia.exe"]));
    });
    await waitFor(() =>
      expect(notifyMock).toHaveBeenCalledWith(
        expect.objectContaining({ tone: "error" }),
      ),
    );
  });

  it("refreshes process usage and survives refresh failures", async () => {
    scanApiMock.getRunningProcesses
      .mockResolvedValueOnce([
        {
          pid: 1,
          name: "SampleApp.exe",
          path: "C:\\Games\\SampleApp.exe",
          cpu_usage: 12,
          gpu_usage: 0,
        },
      ])
      .mockResolvedValueOnce([
        {
          pid: 1,
          name: "SampleApp.exe",
          path: "C:\\Games\\SampleApp.exe",
          cpu_usage: 99,
          gpu_usage: 0,
        },
      ])
      .mockRejectedValueOnce(new Error("refresh-fail"));

    gamesApiMock.existsByPath.mockResolvedValue(false);
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<Scan />);

    await userEvent.click(screen.getByRole("button", { name: /Процессы/ }));
    await waitFor(() =>
      expect(scanApiMock.getRunningProcesses).toHaveBeenCalledTimes(1),
    );

    const refreshUsageButton = screen.getByTitle(/нагрузку/i);

    await userEvent.click(refreshUsageButton);
    await waitFor(() =>
      expect(scanApiMock.getRunningProcesses).toHaveBeenCalledTimes(2),
    );

    await userEvent.click(refreshUsageButton);
    await waitFor(() =>
      expect(scanApiMock.getRunningProcesses).toHaveBeenCalledTimes(3),
    );

    errorSpy.mockRestore();
  });

  it("clears the filter", async () => {
    scanApiMock.getRunningProcesses.mockResolvedValueOnce([
      {
        pid: 1,
        name: "SampleApp.exe",
        path: "C:\\Games\\SampleApp.exe",
        cpu_usage: 12,
        gpu_usage: 0,
      },
    ]);
    gamesApiMock.existsByPath.mockResolvedValue(false);

    render(<Scan />);

    await userEvent.click(screen.getByRole("button", { name: /Процессы/ }));

    const input = await screen.findByPlaceholderText(/Поиск/);
    await userEvent.type(input, "x");
    expect(input).toHaveValue("x");

    const clear = document.querySelector("button > svg.lucide-x");
    if (clear) {
      fireEvent.change(input, { target: { value: "" } });
    }

    expect(input).toHaveValue("");
  });
});
