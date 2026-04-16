import { act, render, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import {
  GamesProvider,
  useGames,
  useGamesActions,
  useGamesState,
} from "@/store/GamesContext";
import {
  createTestGame,
  testFavoriteGameFixture,
  testGameFixture,
} from "@/types";

const gamesApiMock = vi.hoisted(() => ({
  getAll: vi.fn(),
  add: vi.fn(),
  addBatch: vi.fn(),
  update: vi.fn(),
  delete: vi.fn(),
  toggleFavorite: vi.fn(),
  search: vi.fn(),
}));

vi.mock("@/lib/api", () => ({ gamesApi: gamesApiMock }));

type GamesContextValue = ReturnType<typeof useGames>;

const ContextHarness = ({
  onContext,
}: {
  onContext: (context: GamesContextValue) => void;
}) => {
  const contextValue = useGames();

  useEffect(() => {
    onContext(contextValue);
  }, [contextValue, onContext]);

  return (
    <div>
      <div data-testid="loading">
        {contextValue.loading ? "loading" : "ready"}
      </div>
      <div data-testid="error">{contextValue.error ?? ""}</div>
      <ul data-testid="games">
        {contextValue.games.map((game) => (
          <li key={game.id} data-testid="game-item">
            {game.name}
          </li>
        ))}
      </ul>
      <ul data-testid="favorites">
        {contextValue.favorites.map((game) => (
          <li key={game.id} data-testid="favorite-item">
            {game.name}
          </li>
        ))}
      </ul>
    </div>
  );
};

describe("GamesContext", () => {
  let latestContext: GamesContextValue | null = null;

  const handleContext = (context: GamesContextValue) => {
    latestContext = context;
  };

  beforeEach(() => {
    latestContext = null;
    vi.clearAllMocks();
  });

  it("throws if hooks are used outside GamesProvider", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    function BadState() {
      useGamesState();
      return null;
    }

    function BadActions() {
      useGamesActions();
      return null;
    }

    expect(() => render(<BadState />)).toThrow(
      "useGamesState must be used within GamesProvider",
    );
    expect(() => render(<BadActions />)).toThrow(
      "useGamesActions must be used within GamesProvider",
    );

    spy.mockRestore();
  });

  it("loads games and derives favorites", async () => {
    gamesApiMock.getAll.mockResolvedValueOnce([
      testFavoriteGameFixture,
      testGameFixture,
    ]);

    render(
      <GamesProvider>
        <ContextHarness onContext={handleContext} />
      </GamesProvider>,
    );

    expect(screen.getByTestId("loading")).toHaveTextContent("loading");

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("ready"),
    );

    expect(screen.getByTestId("games")).toHaveTextContent(testGameFixture.name);
    expect(screen.getByTestId("games")).toHaveTextContent(
      testFavoriteGameFixture.name,
    );
    expect(screen.getByTestId("favorites")).toHaveTextContent(
      testFavoriteGameFixture.name,
    );

    // getGame helper.
    expect(latestContext?.getGame(testGameFixture.id)).toEqual(testGameFixture);
    expect(latestContext?.getGame("missing")).toBeUndefined();
  });

  it("adds games and keeps list sorted", async () => {
    const initialGames = [
      createTestGame({ id: "game-3", name: "Zebra" }),
      createTestGame({ id: "game-4", name: "Arcadia" }),
    ];
    const addedGame = createTestGame({ id: "game-5", name: "Apex" });

    gamesApiMock.getAll.mockResolvedValueOnce(initialGames);
    gamesApiMock.add.mockResolvedValueOnce(addedGame);

    render(
      <GamesProvider>
        <ContextHarness onContext={handleContext} />
      </GamesProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("ready"),
    );

    await act(async () => {
      await latestContext?.addGame({
        name: addedGame.name,
        exe_path: addedGame.exe_path,
        exe_name: addedGame.exe_name,
      });
    });

    const gameNames = screen
      .getAllByTestId("game-item")
      .map((item) => item.textContent);

    expect(gameNames).toEqual(["Apex", "Arcadia", "Zebra"]);
  });

  it("updates favorites and skips empty searches", async () => {
    const baseGame = createTestGame({ id: "game-6", name: "Nova" });
    const favoriteGame = createTestGame({
      id: "game-7",
      name: "Pulse",
      is_favorite: true,
    });

    gamesApiMock.getAll.mockResolvedValueOnce([baseGame, favoriteGame]);
    gamesApiMock.toggleFavorite.mockResolvedValueOnce({
      ...baseGame,
      is_favorite: true,
    });

    render(
      <GamesProvider>
        <ContextHarness onContext={handleContext} />
      </GamesProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("ready"),
    );

    await act(async () => {
      await latestContext?.toggleFavorite(baseGame.id);
    });

    const favoriteNames = screen
      .getAllByTestId("favorite-item")
      .map((item) => item.textContent);

    expect(favoriteNames).toEqual(["Nova", "Pulse"]);

    const searchResult = await latestContext?.searchGames("   ");
    expect(gamesApiMock.search).not.toHaveBeenCalled();
    expect(searchResult).toEqual([
      { ...baseGame, is_favorite: true },
      favoriteGame,
    ]);
  });

  it("updates and deletes games, and delegates search", async () => {
    const gameA = createTestGame({ id: "game-a", name: "Arcadia" });
    const gameB = createTestGame({ id: "game-b", name: "Borealis" });

    gamesApiMock.getAll.mockResolvedValueOnce([gameA, gameB]);
    gamesApiMock.update.mockResolvedValueOnce({ ...gameA, name: "Arcadia X" });
    gamesApiMock.delete.mockResolvedValueOnce(undefined);
    gamesApiMock.search.mockResolvedValueOnce([gameB]);

    render(
      <GamesProvider>
        <ContextHarness onContext={handleContext} />
      </GamesProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("ready"),
    );

    await act(async () => {
      await latestContext?.updateGame(gameA.id, { name: "Arcadia X" });
    });
    expect(screen.getByTestId("games")).toHaveTextContent("Arcadia X");

    await act(async () => {
      await latestContext?.deleteGame(gameB.id);
    });
    expect(screen.getByTestId("games")).not.toHaveTextContent("Borealis");

    const result = await latestContext?.searchGames("query");
    expect(gamesApiMock.search).toHaveBeenCalledWith("query");
    expect(result).toEqual([gameB]);
  });

  it("adds games in batch (including empty batches)", async () => {
    const initialGames = [
      createTestGame({ id: "game-1", name: "Zebra" }),
      createTestGame({ id: "game-2", name: "Arcadia" }),
    ];

    gamesApiMock.getAll.mockResolvedValueOnce(initialGames);
    gamesApiMock.addBatch.mockResolvedValueOnce([]);
    gamesApiMock.addBatch.mockResolvedValueOnce([
      createTestGame({ id: "game-3", name: "Apex" }),
    ]);

    render(
      <GamesProvider>
        <ContextHarness onContext={handleContext} />
      </GamesProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("ready"),
    );

    await act(async () => {
      await latestContext?.addGames([]);
    });

    // Empty result should not change list
    const namesBefore = screen
      .getAllByTestId("game-item")
      .map((item) => item.textContent);
    expect(namesBefore).toEqual(["Zebra", "Arcadia"]);

    await act(async () => {
      await latestContext?.addGames([
        { name: "Apex", exe_path: "C:\\Apex.exe", exe_name: "Apex.exe" },
      ]);
    });

    const namesAfter = screen
      .getAllByTestId("game-item")
      .map((item) => item.textContent);
    expect(namesAfter).toEqual(["Apex", "Arcadia", "Zebra"]);
  });

  it("surfaces refresh errors", async () => {
    gamesApiMock.getAll.mockRejectedValueOnce(new Error("boom"));
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(
      <GamesProvider>
        <ContextHarness onContext={handleContext} />
      </GamesProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("ready"),
    );
    expect(screen.getByTestId("error")).toHaveTextContent("boom");

    errorSpy.mockRestore();
  });

  it("uses a generic message when refresh throws a non-Error value", async () => {
    gamesApiMock.getAll.mockRejectedValueOnce("boom");
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(
      <GamesProvider>
        <ContextHarness onContext={handleContext} />
      </GamesProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("loading")).toHaveTextContent("ready"),
    );

    expect(screen.getByTestId("error")).toHaveTextContent(
      "Failed to load games",
    );

    errorSpy.mockRestore();
  });
});
