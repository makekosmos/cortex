import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import {
  SIDEBAR_STORAGE_KEY,
  Sidebar,
} from "@/components/Sidebar";
import { useGamesState } from "@/store/GamesContext";
import { createTestGame } from "@/types";

vi.mock("@/store/GamesContext", () => ({
  useGamesState: vi.fn(),
}));
vi.mock("@/components/Spotlight", () => ({
  default: ({
    triggerVariant,
    triggerClassName,
    enableShortcut = true,
  }: {
    triggerVariant?: string;
    triggerClassName?: string;
    enableShortcut?: boolean;
  }) => (
    <button
      type="button"
      data-testid="sidebar-spotlight-trigger"
      data-trigger-variant={triggerVariant}
      data-trigger-class={triggerClassName}
      data-shortcut-enabled={enableShortcut ? "true" : "false"}
    >
      Search
    </button>
  ),
}));

const useGamesStateMock = vi.mocked(useGamesState);

describe("components/Sidebar", () => {
  beforeEach(() => {
    localStorage.clear();
    useGamesStateMock.mockReset();
  });

  it("renders favorites section and wires the sidebar search trigger props", () => {
    const favorites = Array.from({ length: 6 }).map((_, i) =>
      createTestGame({ id: `g${i}`, name: `Game ${i}`, is_favorite: true }),
    );
    useGamesStateMock.mockReturnValue({ favorites } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar mobile />
      </MemoryRouter>,
    );

    expect(screen.getByText("Game 0")).toBeInTheDocument();
    expect(screen.getByText("Game 4")).toBeInTheDocument();
    expect(screen.queryByText("Game 5")).not.toBeInTheDocument();
    expect(screen.getByText(/\+1/)).toBeInTheDocument();
    expect(screen.getByTestId("sidebar-spotlight-trigger")).toHaveAttribute(
      "data-trigger-variant",
      "sidebar",
    );
    expect(screen.getByTestId("sidebar-spotlight-trigger")).toHaveAttribute(
      "data-trigger-class",
      "arrancador-sidebar-search-button",
    );
    expect(screen.getByTestId("sidebar-spotlight-trigger")).toHaveAttribute(
      "data-shortcut-enabled",
      "false",
    );
  });

  it("persists hidden config to localStorage without dropping the current width", async () => {
    useGamesStateMock.mockReturnValue({ favorites: [] } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar initialConfig={{ width: 240, hidden: false }} />
      </MemoryRouter>,
    );

    await userEvent.click(screen.getByTestId("sidebar-toggle"));

    await waitFor(() => {
      const raw = localStorage.getItem(SIDEBAR_STORAGE_KEY);
      expect(raw).toBeTruthy();
      expect(JSON.parse(raw ?? "{}")).toMatchObject({
        width: 240,
        hidden: true,
      });
    });
  });

  it("initializes hidden state from localStorage config", () => {
    localStorage.setItem(
      SIDEBAR_STORAGE_KEY,
      JSON.stringify({ width: 220, hidden: true }),
    );
    useGamesStateMock.mockReturnValue({ favorites: [] } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar />
      </MemoryRouter>,
    );

    expect(screen.getByTestId("kepler-sidebar").className).toContain("is-hidden");
  });

  it("sanitizes persisted sidebar width before rendering desktop sidebar", () => {
    localStorage.setItem(
      SIDEBAR_STORAGE_KEY,
      JSON.stringify({ width: 0, hidden: false }),
    );
    useGamesStateMock.mockReturnValue({ favorites: [] } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar />
      </MemoryRouter>,
    );

    expect(screen.getByTestId("kepler-sidebar")).toHaveStyle({ width: "160px" });
    expect(screen.getByRole("link", { name: "Библиотека" })).toBeInTheDocument();
  });

  it("tolerates localStorage failures", async () => {
    const getItemSpy = vi
      .spyOn(Storage.prototype, "getItem")
      .mockImplementation(() => {
        throw new Error("blocked");
      });
    const setItemSpy = vi
      .spyOn(Storage.prototype, "setItem")
      .mockImplementation(() => {
        throw new Error("blocked");
      });

    try {
      useGamesStateMock.mockReturnValue({ favorites: [] } as never);

      render(
        <MemoryRouter initialEntries={["/"]}>
          <Sidebar mobile />
        </MemoryRouter>,
      );

      expect(screen.getByText("Библиотека")).toBeInTheDocument();
      expect(getItemSpy).toHaveBeenCalled();

      render(
        <MemoryRouter initialEntries={["/"]}>
          <Sidebar />
        </MemoryRouter>,
      );

      await userEvent.click(screen.getAllByTestId("sidebar-toggle")[0]!);
      await waitFor(() => expect(setItemSpy).toHaveBeenCalled());
    } finally {
      getItemSpy.mockRestore();
      setItemSpy.mockRestore();
    }
  });

  it("highlights the active favorite route", () => {
    const favorites = [
      createTestGame({ id: "g1", name: "Fav", is_favorite: true }),
    ];
    useGamesStateMock.mockReturnValue({ favorites } as never);

    render(
      <MemoryRouter initialEntries={["/game/g1"]}>
        <Sidebar mobile />
      </MemoryRouter>,
    );

    const favLink = screen.getByRole("link", { name: "Fav" });
    expect(favLink.className).toContain("kepler-sidebar-project-link--active");
  });

  it("renders settings footer item", () => {
    useGamesStateMock.mockReturnValue({ favorites: [] } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar mobile />
      </MemoryRouter>,
    );

    const settingsLink = screen.getByRole("link", { name: "Настройки" });
    expect(settingsLink.getAttribute("href")).toBe("/settings");
  });

  it("keeps search in sidebar even when the toggle button is disabled", () => {
    useGamesStateMock.mockReturnValue({ favorites: [] } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar mobile showToggle={false} />
      </MemoryRouter>,
    );

    expect(screen.getByTestId("sidebar-spotlight-trigger")).toBeInTheDocument();
    expect(screen.queryByTestId("sidebar-toggle")).not.toBeInTheDocument();
  });
});
