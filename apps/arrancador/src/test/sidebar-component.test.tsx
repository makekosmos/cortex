import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { Sidebar } from "@/components/Sidebar";
import { useGamesState } from "@/store/GamesContext";
import { createTestGame } from "@/types";

vi.mock("@/store/GamesContext", () => ({
  useGamesState: vi.fn(),
}));

const useGamesStateMock = vi.mocked(useGamesState);

describe("components/Sidebar", () => {
  beforeEach(() => {
    localStorage.clear();
    useGamesStateMock.mockReset();
  });

  it("renders favorites section and truncates to 5 + extra indicator", () => {
    const favorites = Array.from({ length: 6 }).map((_, i) =>
      createTestGame({ id: `g${i}`, name: `Game ${i}`, is_favorite: true }),
    );
    useGamesStateMock.mockReturnValue({ favorites } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar />
      </MemoryRouter>,
    );

    expect(screen.getByText("Game 0")).toBeInTheDocument();
    expect(screen.getByText("Game 4")).toBeInTheDocument();
    expect(screen.queryByText("Game 5")).not.toBeInTheDocument();
    expect(screen.getByText(/\+1/)).toBeInTheDocument();
  });

  it("persists collapsed state to localStorage", async () => {
    useGamesStateMock.mockReturnValue({ favorites: [] } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar />
      </MemoryRouter>,
    );

    expect(localStorage.getItem("arrancador_sidebar_collapsed")).not.toBe(
      "true",
    );

    const buttons = screen.getAllByRole("button");
    const collapseButton = buttons[buttons.length - 1];
    if (!collapseButton) throw new Error("missing collapse button");

    await userEvent.click(collapseButton);
    await waitFor(() =>
      expect(localStorage.getItem("arrancador_sidebar_collapsed")).toBe("true"),
    );
  });

  it("initializes collapsed state from localStorage", () => {
    localStorage.setItem("arrancador_sidebar_collapsed", "true");
    useGamesStateMock.mockReturnValue({ favorites: [] } as never);

    render(
      <MemoryRouter initialEntries={["/game/g1"]}>
        <Sidebar />
      </MemoryRouter>,
    );

    // When collapsed, nav items get a title attribute. Any one is fine.
    const link = screen.getAllByRole("link")[0];
    expect(link.getAttribute("title")).toBeTruthy();
  });

  it("tolerates localStorage failures (getItem/setItem throw)", async () => {
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
          <Sidebar />
        </MemoryRouter>,
      );

      // Still renders even if storage is unavailable.
      expect(screen.getByText("Библиотека")).toBeInTheDocument();
      expect(getItemSpy).toHaveBeenCalled();

      const buttons = screen.getAllByRole("button");
      const collapseButton = buttons[buttons.length - 1];
      if (!collapseButton) throw new Error("missing collapse button");

      await userEvent.click(collapseButton);
      await waitFor(() => expect(setItemSpy).toHaveBeenCalled());
    } finally {
      getItemSpy.mockRestore();
      setItemSpy.mockRestore();
    }
  });

  it("keeps favorites accessible when collapsed while visually hiding the header", () => {
    localStorage.setItem("arrancador_sidebar_collapsed", "true");
    const favorites = [
      createTestGame({ id: "g1", name: "Fav", is_favorite: true }),
    ];
    useGamesStateMock.mockReturnValue({ favorites } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar />
      </MemoryRouter>,
    );

    const header = screen.getByText("Избранное");
    expect(header.className).toContain("lg:sr-only");
    expect(screen.getByRole("link", { name: "Fav" })).toBeInTheDocument();
  });

  it("renders even when localStorage is unavailable (typeof localStorage === 'undefined')", () => {
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      vi.stubGlobal("localStorage", undefined as any);
      useGamesStateMock.mockReturnValue({ favorites: [] } as never);

      render(
        <MemoryRouter initialEntries={["/"]}>
          <Sidebar />
        </MemoryRouter>,
      );

      expect(screen.getByText("Библиотека")).toBeInTheDocument();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("highlights the active favorite route", () => {
    const favorites = [
      createTestGame({ id: "g1", name: "Fav", is_favorite: true }),
    ];
    useGamesStateMock.mockReturnValue({ favorites } as never);

    render(
      <MemoryRouter initialEntries={["/game/g1"]}>
        <Sidebar />
      </MemoryRouter>,
    );

    const favLink = screen.getByRole("link", { name: "Fav" });
    expect(favLink.className).toContain("bg-sidebar-accent");
  });

  it("renders settings quick button in footer", () => {
    useGamesStateMock.mockReturnValue({ favorites: [] } as never);

    render(
      <MemoryRouter initialEntries={["/"]}>
        <Sidebar />
      </MemoryRouter>,
    );

    const settingsLink = screen.getByRole("link", { name: "Настройки" });
    expect(settingsLink.getAttribute("href")).toBe("/settings");
  });
});
