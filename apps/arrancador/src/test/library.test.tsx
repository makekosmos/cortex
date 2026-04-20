import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import Library from "@/pages/Library";
import { useGamesActions, useGamesState } from "@/store/GamesContext";
import { testFavoriteGameFixture, testGameFixture } from "@/types";

const { notifyMock, gamesApiMock } = vi.hoisted(() => ({
  notifyMock: vi.fn(),
  gamesApiMock: {
    existsByPath: vi.fn(),
    isInstalled: vi.fn(),
    resolveShortcutTarget: vi.fn(),
  },
}));

vi.mock("@/store/GamesContext", () => ({
  useGamesState: vi.fn(),
  useGamesActions: vi.fn(),
}));
vi.mock("@/components/GameCard", () => ({
  GameCard: (props: { game: { id: string; name: string } }) => (
    <a href={`/games/${props.game.id}`}>{props.game.name}</a>
  ),
}));
vi.mock("@/components/ToastProvider", () => ({
  useToast: () => ({ notify: notifyMock }),
}));
vi.mock("@/lib/api", () => ({
  gamesApi: gamesApiMock,
  metadataApi: { getApiKey: vi.fn(), search: vi.fn(), apply: vi.fn() },
}));
vi.mock("@/components/RawgMetadataPrompt", () => ({
  RawgMetadataPrompt: (props: {
    game: { id: string; name: string };
    remaining: number;
    onNext: () => void;
    onSkipAll: () => void;
  }) => (
    <div>
      <div>RAWG: {props.game.name}</div>
      <button type="button" onClick={props.onNext}>
        next
      </button>
      <button type="button" onClick={props.onSkipAll}>
        skip-all
      </button>
    </div>
  ),
}));

const useGamesStateMock = vi.mocked(useGamesState);
const useGamesActionsMock = vi.mocked(useGamesActions);

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

const getDropTarget = (container: HTMLElement) => {
  const target = container.firstElementChild as HTMLElement | null;
  if (!target) {
    throw new Error("missing drop target");
  }
  return target;
};

const renderLibrary = () =>
  render(
    <MemoryRouter>
      <Library />
    </MemoryRouter>,
  );

describe("Library", () => {
  beforeEach(() => {
    window.localStorage.clear();
    useGamesStateMock.mockReset();
    useGamesActionsMock.mockReset();
    notifyMock.mockReset();
    gamesApiMock.existsByPath.mockReset();
    gamesApiMock.isInstalled.mockReset();
    gamesApiMock.resolveShortcutTarget.mockReset();
    gamesApiMock.isInstalled.mockResolvedValue(true);

    useGamesActionsMock.mockReturnValue({
      refreshGames: vi.fn(),
      addGame: vi.fn(),
      addGames: vi.fn().mockResolvedValue([]),
      updateGame: vi.fn(),
      deleteGame: vi.fn(),
      toggleFavorite: vi.fn(),
      searchGames: vi.fn(),
    });
  });

  it("renders icon header controls and centered view mode toggle", async () => {
    useGamesStateMock.mockReturnValue({
      games: [testGameFixture, testFavoriteGameFixture],
      favorites: [testFavoriteGameFixture],
      loading: false,
      error: null,
      getGame: vi.fn(),
    });

    renderLibrary();

    expect(screen.queryByPlaceholderText(/поиск игр/i)).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Фильтры" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Сортировка" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Сетка" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Список" }),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Фильтры" }));

    expect(screen.getByText("Жанры")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Избранное" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Сбросить" }),
    ).toBeInTheDocument();
  });

  it("shows favorites when toggled from the filters panel", async () => {
    useGamesStateMock.mockReturnValue({
      games: [testGameFixture, testFavoriteGameFixture],
      favorites: [testFavoriteGameFixture],
      loading: false,
      error: null,
      getGame: vi.fn(),
    });

    renderLibrary();

    await userEvent.click(screen.getByRole("button", { name: "Фильтры" }));
    await userEvent.click(screen.getByRole("button", { name: "Избранное" }));

    expect(
      screen.queryByRole("link", { name: testGameFixture.name }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: testFavoriteGameFixture.name }),
    ).toBeInTheDocument();
  });

  it("toggles the drop overlay based on supported drag entries", async () => {
    useGamesStateMock.mockReturnValue({
      games: [testGameFixture],
      favorites: [],
      loading: false,
      error: null,
      getGame: vi.fn(),
    });

    const { container } = renderLibrary();
    const dropTarget = getDropTarget(container);

    fireEvent.dragEnter(dropTarget, {
      dataTransfer: { types: ["Files"] },
    });
    expect(
      await screen.findByText(/отпустите, чтобы добавить игру/i),
    ).toBeInTheDocument();

    fireEvent.dragOver(dropTarget, {
      dataTransfer: { types: ["Files"] },
    });
    expect(screen.getByText(/отпустите, чтобы добавить игру/i)).toBeInTheDocument();

    fireEvent.dragLeave(dropTarget, {
      dataTransfer: { types: ["Files"] },
    });
    await waitFor(() =>
      expect(
        screen.queryByText(/отпустите, чтобы добавить игру/i),
      ).not.toBeInTheDocument(),
    );
  });

  it("adds dropped .exe/.lnk paths, skips existing/duplicates, and enqueues RAWG prompt", async () => {
    const addGames = vi
      .fn()
      .mockResolvedValue([
        { ...testGameFixture, id: "added-1", name: "Added Game" },
      ]);

    useGamesActionsMock.mockReturnValue({
      refreshGames: vi.fn(),
      addGame: vi.fn(),
      addGames,
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

    gamesApiMock.resolveShortcutTarget.mockResolvedValueOnce(
      "C:\\Games\\ShortcutTarget.exe",
    );
    gamesApiMock.existsByPath
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true);

    const { container } = renderLibrary();
    const dropTarget = getDropTarget(container);

    fireEvent.drop(dropTarget, {
      dataTransfer: {
        types: ["Files"],
        files: [
          createDroppedFile("C:\\Games\\Game.exe"),
          createDroppedFile("C:\\Games\\Game.exe"),
          createDroppedFile("C:\\Games\\Shortcut.lnk"),
          createDroppedFile("C:\\Games\\Readme.txt"),
        ],
      },
    });

    await waitFor(() => expect(addGames).toHaveBeenCalled());
    expect(notifyMock).toHaveBeenCalledWith(
      expect.objectContaining({ tone: "success" }),
    );

    expect(await screen.findByText("RAWG: Added Game")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "next" }));
  });

  it("notifies when dropped paths contain nothing addable", async () => {
    useGamesStateMock.mockReturnValue({
      games: [],
      favorites: [],
      loading: false,
      error: null,
      getGame: vi.fn(),
    });

    const { container } = renderLibrary();
    const dropTarget = getDropTarget(container);

    fireEvent.drop(dropTarget, {
      dataTransfer: {
        types: ["Files"],
        files: [createDroppedFile("C:\\tmp\\note.txt")],
      },
    });

    await waitFor(() =>
      expect(notifyMock).toHaveBeenCalledWith(
        expect.objectContaining({ tone: "warning" }),
      ),
    );
  });

  it("surfaces shortcut resolution errors and addGames failures", async () => {
    const addGames = vi.fn().mockRejectedValue(new Error("nope"));

    useGamesActionsMock.mockReturnValue({
      refreshGames: vi.fn(),
      addGame: vi.fn(),
      addGames,
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

    gamesApiMock.resolveShortcutTarget.mockRejectedValueOnce(
      new Error("bad lnk"),
    );
    gamesApiMock.existsByPath.mockResolvedValue(false);

    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const { container } = renderLibrary();
    const dropTarget = getDropTarget(container);

    fireEvent.drop(dropTarget, {
      dataTransfer: {
        types: ["Files"],
        files: [
          createDroppedFile("C:\\Games\\BadShortcut.lnk"),
          createDroppedFile("C:\\Games\\Ok.exe"),
        ],
      },
    });

    await waitFor(() =>
      expect(notifyMock).toHaveBeenCalledWith(
        expect.objectContaining({ tone: "error" }),
      ),
    );

    errorSpy.mockRestore();
  });
});
