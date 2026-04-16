import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import Spotlight from "@/components/Spotlight";
import { useLanguage } from "@/components/language-provider";
import { useGamesState } from "@/store/GamesContext";
import { createTestGame } from "@/types";

const { navigateMock } = vi.hoisted(() => ({
  navigateMock: vi.fn(),
}));

vi.mock("react-router-dom", () => ({
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
  beforeEach(() => {
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

  it("closes on Escape and removes its document listener on unmount", async () => {
    const addSpy = vi.spyOn(document, "addEventListener");
    const removeSpy = vi.spyOn(document, "removeEventListener");

    const { unmount } = render(<Spotlight showTrigger={false} />);

    expect(addSpy).toHaveBeenCalledWith("keydown", expect.any(Function));

    await act(async () => {
      fireEvent.keyDown(document, {
        key: "k",
        code: "KeyK",
        ctrlKey: true,
      });
    });
    expect(await screen.findByPlaceholderText("Search games...")).toBeInTheDocument();

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    });
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
});
