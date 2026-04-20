import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import GameDetail from "@/pages/GameDetail";
import { useGamesActions, useGamesState } from "@/store/GamesContext";
import { createTestGame } from "@/types";

const {
  toggleFavoriteMock,
  refreshGamesMock,
  deleteGameMock,
  gamesApiMock,
  backupApiMock,
  metadataApiMock,
} = vi.hoisted(() => ({
  toggleFavoriteMock: vi.fn(),
  refreshGamesMock: vi.fn(),
  deleteGameMock: vi.fn(),
  gamesApiMock: {
    isInstalled: vi.fn(),
    getRunningInstances: vi.fn(),
    killProcesses: vi.fn(),
    launch: vi.fn(),
    update: vi.fn(),
  },
  backupApiMock: {
    getForGame: vi.fn(),
    checkRestoreNeeded: vi.fn(),
    shouldBackupBeforeLaunch: vi.fn(),
    checkBackupNeeded: vi.fn(),
    create: vi.fn(),
    restore: vi.fn(),
    delete: vi.fn(),
  },
  metadataApiMock: {
    search: vi.fn(),
    getDetails: vi.fn(),
    apply: vi.fn(),
  },
}));

vi.mock("@/store/GamesContext", () => ({
  useGamesActions: vi.fn(),
  useGamesState: vi.fn(),
}));
vi.mock("@/lib/api", () => ({
  gamesApi: gamesApiMock,
  backupApi: backupApiMock,
  metadataApi: metadataApiMock,
}));
vi.mock("@/components/ToastProvider", () => ({
  useToast: () => ({ notify: vi.fn() }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn() }));

const useGamesActionsMock = vi.mocked(useGamesActions);
const useGamesStateMock = vi.mocked(useGamesState);

describe("GameDetail", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    gamesApiMock.isInstalled.mockResolvedValue(true);
    gamesApiMock.getRunningInstances.mockResolvedValue(0);
    backupApiMock.getForGame.mockResolvedValue([]);
    backupApiMock.checkRestoreNeeded.mockResolvedValue({
      should_restore: false,
      backup_id: null,
      current_size: 0,
      backup_size: 0,
    });
  });

  it("renders details and toggles favorites", async () => {
    const game = createTestGame({ id: "game-1", name: "Arcadia" });
    toggleFavoriteMock.mockResolvedValue({ ...game, is_favorite: true });

    useGamesStateMock.mockReturnValue({
      games: [game],
      favorites: [],
      loading: false,
      error: null,
      getGame: vi.fn(),
    });
    useGamesActionsMock.mockReturnValue({
      refreshGames: refreshGamesMock,
      addGame: vi.fn(),
      addGames: vi.fn(),
      updateGame: vi.fn(),
      deleteGame: deleteGameMock,
      toggleFavorite: toggleFavoriteMock,
      searchGames: vi.fn(),
    });

    render(
      <MemoryRouter initialEntries={["/game/game-1"]}>
        <Routes>
          <Route path="/game/:id" element={<GameDetail />} />
        </Routes>
      </MemoryRouter>,
    );

    await waitFor(() => expect(screen.getByText(game.name)).toBeInTheDocument());

    const favoriteButton = screen.getByTitle("В избранное");
    await userEvent.click(favoriteButton);

    expect(toggleFavoriteMock).toHaveBeenCalledWith("game-1");
  });

  it("renders the hero copy, shows the compact description, and opens the full description modal", async () => {
    const game = createTestGame({
      id: "game-hero",
      name: "Night Runner",
      background_image: "https://example.test/night-runner-wide.jpg",
      description:
        "A precision shooter about impossible angles, split-second reactions, and late-night ranked matches.",
      released: "2020-06-02",
      total_playtime: 19800,
      genres: "Action, RPG, Indie",
    });

    useGamesStateMock.mockReturnValue({
      games: [game],
      favorites: [],
      loading: false,
      error: null,
      getGame: vi.fn(),
    });
    useGamesActionsMock.mockReturnValue({
      refreshGames: refreshGamesMock,
      addGame: vi.fn(),
      addGames: vi.fn(),
      updateGame: vi.fn(),
      deleteGame: deleteGameMock,
      toggleFavorite: toggleFavoriteMock,
      searchGames: vi.fn(),
    });

    render(
      <MemoryRouter initialEntries={["/game/game-hero"]}>
        <Routes>
          <Route path="/game/:id" element={<GameDetail />} />
        </Routes>
      </MemoryRouter>,
    );

    const hero = await screen.findByTestId("game-detail-hero");
    const heroCopy = screen.getByTestId("game-detail-hero-copy");
    const heroActions = screen.getByTestId("game-detail-hero-actions");
    const description = screen.getByTestId("game-detail-hero-description");
    const descriptionButton = screen.getByTestId("game-detail-description-button");
    const playButton = screen.getByRole("button", { name: "Играть" });

    expect(hero.className).toContain("h-[80vh]");
    expect(hero.className).toContain("rounded-b-[32px]");
    expect(hero).toContainElement(screen.getByRole("heading", { name: game.name }));
    expect(hero).toContainElement(playButton);
    expect(heroCopy).toHaveTextContent("Экшен · RPG · Инди");
    expect(description.className).toContain("truncate-2");
    expect(heroCopy).toHaveTextContent(
      "A precision shooter about impossible angles, split-second reactions, and late-night ranked matches.",
    );
    expect(heroCopy).toHaveTextContent("2020 · 5 ч");
    expect(heroActions).toContainElement(playButton);

    await userEvent.click(descriptionButton);

    expect(await screen.findByTestId("game-detail-description-modal")).toBeInTheDocument();
    expect(screen.getByText("Описание игры")).toBeInTheDocument();
  });

  it.fails(
    "waits for backup completion before launching when backup is required",
    async () => {
      const game = createTestGame({
        id: "game-1",
        name: "Arcadia",
        backup_enabled: true,
      });
      let resolveBackup: (() => void) | undefined;

      gamesApiMock.launch.mockResolvedValue(undefined);
      backupApiMock.checkRestoreNeeded.mockResolvedValue({
        should_restore: false,
        backup_id: null,
        current_size: 0,
        backup_size: 0,
      });
      backupApiMock.shouldBackupBeforeLaunch.mockResolvedValue(true);
      backupApiMock.checkBackupNeeded.mockResolvedValue(true);
      backupApiMock.create.mockReturnValueOnce(
        new Promise<void>((resolve) => {
          resolveBackup = resolve;
        }),
      );

      useGamesStateMock.mockReturnValue({
        games: [game],
        favorites: [],
        loading: false,
        error: null,
        getGame: vi.fn(),
      });
      useGamesActionsMock.mockReturnValue({
        refreshGames: refreshGamesMock,
        addGame: vi.fn(),
        addGames: vi.fn(),
        updateGame: vi.fn(),
        deleteGame: deleteGameMock,
        toggleFavorite: toggleFavoriteMock,
        searchGames: vi.fn(),
      });

      render(
        <MemoryRouter initialEntries={["/game/game-1"]}>
          <Routes>
            <Route path="/game/:id" element={<GameDetail />} />
          </Routes>
        </MemoryRouter>,
      );

      await waitFor(() => expect(screen.getByText(game.name)).toBeInTheDocument());
      await userEvent.click(screen.getByRole("button", { name: "Играть" }));
      expect(await screen.findByText("Создать бэкап?")).toBeInTheDocument();
      await userEvent.click(
        screen.getByRole("button", { name: "Бэкап и Запуск" }),
      );

      await waitFor(() => expect(gamesApiMock.launch).toHaveBeenCalled());
      if (resolveBackup) resolveBackup();
      expect(gamesApiMock.launch).not.toHaveBeenCalled();
    },
  );

  it.fails(
    "does not launch when restore fails",
    async () => {
      const game = createTestGame({
        id: "game-1",
        name: "Arcadia",
        backup_enabled: true,
      });

      gamesApiMock.launch.mockResolvedValue(undefined);
      backupApiMock.checkRestoreNeeded.mockResolvedValue({
        should_restore: true,
        backup_id: "backup-1",
        current_size: 1024,
        backup_size: 2048,
      });
      backupApiMock.shouldBackupBeforeLaunch.mockResolvedValue(false);
      backupApiMock.checkBackupNeeded.mockResolvedValue(false);
      backupApiMock.restore.mockRejectedValueOnce(new Error("restore failed"));

      useGamesStateMock.mockReturnValue({
        games: [game],
        favorites: [],
        loading: false,
        error: null,
        getGame: vi.fn(),
      });
      useGamesActionsMock.mockReturnValue({
        refreshGames: refreshGamesMock,
        addGame: vi.fn(),
        addGames: vi.fn(),
        updateGame: vi.fn(),
        deleteGame: deleteGameMock,
        toggleFavorite: toggleFavoriteMock,
        searchGames: vi.fn(),
      });

      render(
        <MemoryRouter initialEntries={["/game/game-1"]}>
          <Routes>
            <Route path="/game/:id" element={<GameDetail />} />
          </Routes>
        </MemoryRouter>,
      );

      await waitFor(() => expect(screen.getByText(game.name)).toBeInTheDocument());
      await userEvent.click(screen.getByRole("button", { name: "Играть" }));
      expect(await screen.findByText("Восстановить бэкап?")).toBeInTheDocument();
      await userEvent.click(
        screen.getByRole("button", { name: "Восстановить и запустить" }),
      );

      await waitFor(() => expect(gamesApiMock.launch).toHaveBeenCalled());
      expect(gamesApiMock.launch).not.toHaveBeenCalled();
    },
  );
});
