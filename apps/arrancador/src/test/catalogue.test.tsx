import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import CataloguePage from "@/pages/Catalogue";
import { useGamesActions, useGamesState } from "@/store/GamesContext";
import { createTestGame } from "@/types";

const { notifyMock, gamesActionsMock, metadataApiMock } = vi.hoisted(() => ({
  notifyMock: vi.fn(),
  gamesActionsMock: {
    addGame: vi.fn(),
    refreshGames: vi.fn(),
  },
  metadataApiMock: {
    search: vi.fn(),
    apply: vi.fn(),
  },
}));

vi.mock("@/components/ToastProvider", () => ({
  useToast: () => ({ notify: notifyMock }),
}));
vi.mock("@/lib/api", () => ({
  metadataApi: metadataApiMock,
}));
vi.mock("@/store/GamesContext", () => ({
  useGamesState: vi.fn(),
  useGamesActions: vi.fn(),
}));

const useGamesStateMock = vi.mocked(useGamesState);
const useGamesActionsMock = vi.mocked(useGamesActions);

describe("CataloguePage", () => {
  beforeEach(() => {
    notifyMock.mockReset();
    gamesActionsMock.addGame.mockReset();
    gamesActionsMock.refreshGames.mockReset();
    metadataApiMock.search.mockReset();
    metadataApiMock.apply.mockReset();
    useGamesStateMock.mockReset();
    useGamesActionsMock.mockReset();

    useGamesActionsMock.mockReturnValue(gamesActionsMock as never);
    useGamesStateMock.mockReturnValue({
      games: [],
      favorites: [],
      loading: false,
      error: null,
      getGame: vi.fn(),
    } as ReturnType<typeof useGamesState>);
  });

  it("loads the showcase, searches, and adds a RAWG game to the library", async () => {
    const showcase = [
      {
        id: 1101,
        name: "Arcadia",
        slug: "arcadia",
        released: "2024-01-01",
        background_image: null,
        metacritic: 86,
        rating: 4.3,
        ratings_count: null,
        genres: [{ id: 1, name: "Action", slug: "action" }],
        platforms: null,
      },
    ];
    const searched = [
      {
        ...showcase[0],
        id: 2202,
        name: "Bastion",
        slug: "bastion",
      },
    ];

    metadataApiMock.search.mockImplementation((query: string) =>
      Promise.resolve(query.trim() ? searched : showcase),
    );
    gamesActionsMock.addGame.mockResolvedValue(
      createTestGame({
        id: "game-rawg-1",
        name: "Bastion",
        exe_name: "Bastion.exe",
        exe_path: "C:\\Arrancador\\RAWG\\2202\\Bastion.exe",
      }),
    );
    metadataApiMock.apply.mockResolvedValue(
      createTestGame({
        id: "game-rawg-1",
        name: "Bastion",
        exe_name: "Bastion.exe",
        exe_path: "C:\\Arrancador\\RAWG\\2202\\Bastion.exe",
      }),
    );

    render(
      <MemoryRouter>
        <CataloguePage />
      </MemoryRouter>,
    );

    expect(await screen.findByText("Arcadia")).toBeInTheDocument();
    expect(screen.getByText("2024 | Экшен")).toBeInTheDocument();

    const user = userEvent.setup();
    await user.type(screen.getByRole("textbox"), "Bastion{enter}");

    expect(await screen.findByText("Bastion")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: /Add to library/ }),
    );

    await waitFor(() =>
      expect(gamesActionsMock.addGame).toHaveBeenCalledWith({
        name: "Bastion",
        exe_name: "Bastion.exe",
        exe_path: "C:\\Arrancador\\RAWG\\2202\\Bastion.exe",
      }),
    );
    expect(metadataApiMock.apply).toHaveBeenCalledWith("game-rawg-1", 2202, true);
    expect(gamesActionsMock.refreshGames).toHaveBeenCalled();
    expect(notifyMock).toHaveBeenCalledWith(
      expect.objectContaining({ tone: "success" }),
    );
  });

  it("shows an open link for games already in the library", async () => {
    const libraryItem = {
      id: 1101,
      name: "Arcadia",
      slug: "arcadia",
      released: "2024-01-01",
      background_image: null,
      metacritic: 86,
      rating: 4.3,
      ratings_count: null,
      genres: null,
      platforms: null,
    };

    metadataApiMock.search.mockResolvedValue([libraryItem]);
    useGamesStateMock.mockReturnValue({
      games: [
        createTestGame({
          id: "game-1",
          name: "Arcadia",
          rawg_id: 1101,
        }),
      ],
      favorites: [],
      loading: false,
      error: null,
      getGame: vi.fn(),
    } as ReturnType<typeof useGamesState>);

    render(
      <MemoryRouter>
        <CataloguePage />
      </MemoryRouter>,
    );

    expect(await screen.findByText("Arcadia")).toBeInTheDocument();
    expect(
      screen.getByRole("link", {
        name: /Open in library/,
      }),
    ).toHaveAttribute("href", "/game/game-1");
  });
});
