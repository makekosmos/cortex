import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import Spotlight from "@/components/Spotlight";
import { useLanguage } from "@/components/language-provider";
import { useGamesState } from "@/store/GamesContext";
import { createTestGame } from "@/types";

const { locationMock, navigateMock } = vi.hoisted(() => ({
  locationMock: {
    pathname: "/",
    search: "",
    hash: "",
  },
  navigateMock: vi.fn(),
}));

vi.mock("react-router-dom", () => ({
  useLocation: () => locationMock,
  useNavigate: () => navigateMock,
}));
vi.mock("@/components/language-provider", () => ({
  useLanguage: vi.fn(),
}));
vi.mock("@/store/GamesContext", () => ({
  useGamesState: vi.fn(),
}));

const useLanguageMock = vi.mocked(useLanguage);
const useGamesStateMock = vi.mocked(useGamesState);

describe("Spotlight", () => {
  let scrollIntoViewMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    locationMock.pathname = "/";
    locationMock.search = "";
    locationMock.hash = "";
    navigateMock.mockReset();
    useLanguageMock.mockReset();
    useGamesStateMock.mockReset();
    useLanguageMock.mockReturnValue({
      language: "en",
    } as ReturnType<typeof useLanguage>);
    useGamesStateMock.mockReturnValue({
      games: [
        createTestGame({
          id: "game-1",
          name: "Arcadia",
          exe_name: "arcadia.exe",
          is_favorite: false,
          total_playtime: 0,
        }),
        createTestGame({
          id: "game-2",
          name: "Bastion",
          exe_name: "bastion.exe",
          is_favorite: false,
          total_playtime: 5400,
        }),
      ],
      favorites: [],
      loading: false,
      error: null,
      getGame: vi.fn(),
    } as ReturnType<typeof useGamesState>);
    scrollIntoViewMock = vi.fn();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: scrollIntoViewMock as (
        arg?: boolean | ScrollIntoViewOptions,
      ) => void,
    });
  });

  it("opens on Ctrl+K and navigates with the keyboard", async () => {
    const addSpy = vi.spyOn(document, "addEventListener");
    const removeSpy = vi.spyOn(document, "removeEventListener");

    render(<Spotlight showTrigger={false} />);

    expect(addSpy).toHaveBeenCalledWith("keydown", expect.any(Function));

    await act(async () => {
      fireEvent.keyDown(document, {
        key: "k",
        code: "KeyK",
        ctrlKey: true,
      });
    });

    const input = await screen.findByPlaceholderText("Search games...");
    await waitFor(() => expect(input).toHaveFocus());

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
    });
    await waitFor(() =>
      expect(screen.getByRole("option", { name: /Bastion/ })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });

    expect(navigateMock).toHaveBeenCalledWith("/game/game-2");
    expect(removeSpy).toHaveBeenCalled();

    addSpy.mockRestore();
    removeSpy.mockRestore();
  });

  it("resets query and active selection when the shortcut closes and reopens the palette", async () => {
    render(<Spotlight showTrigger={false} />);

    await act(async () => {
      fireEvent.keyDown(document, {
        key: "k",
        code: "KeyK",
        ctrlKey: true,
      });
    });

    const input = await screen.findByPlaceholderText("Search games...");
    await userEvent.type(input, "bas");
    expect(input).toHaveValue("bas");

    await act(async () => {
      fireEvent.keyDown(document, {
        key: "k",
        code: "KeyK",
        ctrlKey: true,
      });
    });

    await waitFor(() =>
      expect(
        screen.queryByPlaceholderText("Search games..."),
      ).not.toBeInTheDocument(),
    );

    await act(async () => {
      fireEvent.keyDown(document, {
        key: "k",
        code: "KeyK",
        ctrlKey: true,
      });
    });

    const reopenedInput = await screen.findByPlaceholderText("Search games...");
    expect(reopenedInput).toHaveValue("");
    expect(screen.getByRole("option", { name: /Arcadia/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("closes on route changes and removes its document listener on unmount", async () => {
    const addSpy = vi.spyOn(document, "addEventListener");
    const removeSpy = vi.spyOn(document, "removeEventListener");

    const { rerender, unmount } = render(<Spotlight showTrigger={false} />);

    expect(addSpy).toHaveBeenCalledWith("keydown", expect.any(Function));

    await act(async () => {
      fireEvent.keyDown(document, {
        key: "k",
        code: "KeyK",
        ctrlKey: true,
      });
    });
    expect(await screen.findByPlaceholderText("Search games...")).toBeInTheDocument();

    locationMock.pathname = "/settings";
    rerender(<Spotlight showTrigger={false} />);

    await waitFor(() =>
      expect(
        screen.queryByPlaceholderText("Search games..."),
      ).not.toBeInTheDocument(),
    );

    const removeCallsBeforeUnmount = removeSpy.mock.calls.length;
    unmount();
    expect(removeSpy.mock.calls.length).toBeGreaterThan(removeCallsBeforeUnmount);

    addSpy.mockRestore();
    removeSpy.mockRestore();
  });

  it("keeps the active option visible while navigating with arrow keys", async () => {
    useGamesStateMock.mockReturnValue({
      games: Array.from({ length: 15 }, (_, index) =>
        createTestGame({
          id: `game-${index}`,
          name: `Game ${String(index).padStart(2, "0")}`,
          exe_name: `game-${index}.exe`,
          is_favorite: false,
          total_playtime: 0,
        }),
      ),
      favorites: [],
      loading: false,
      error: null,
      getGame: vi.fn(),
    } as ReturnType<typeof useGamesState>);

    render(<Spotlight showTrigger={false} />);

    await act(async () => {
      fireEvent.keyDown(document, {
        key: "k",
        code: "KeyK",
        ctrlKey: true,
      });
    });

    await screen.findByPlaceholderText("Search games...");

    scrollIntoViewMock.mockClear();

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
    });

    await waitFor(() => expect(scrollIntoViewMock).toHaveBeenCalled());
    expect(screen.getByRole("option", { name: /Game 01/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });
});
