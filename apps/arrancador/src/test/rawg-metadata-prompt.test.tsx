import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { RawgMetadataPrompt } from "@/components/RawgMetadataPrompt";

const { notifyMock, metadataApiMock } = vi.hoisted(() => ({
  notifyMock: vi.fn(),
  metadataApiMock: {
    getApiKey: vi.fn(),
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

const renderPrompt = (
  props: Partial<React.ComponentProps<typeof RawgMetadataPrompt>> = {},
) =>
  render(
    <MemoryRouter>
      <RawgMetadataPrompt
        game={{ id: "g1", name: "Arcadia" }}
        remaining={2}
        onNext={vi.fn()}
        onSkipAll={vi.fn()}
        {...props}
      />
    </MemoryRouter>,
  );

describe("RawgMetadataPrompt", () => {
  beforeEach(() => {
    notifyMock.mockReset();
    metadataApiMock.getApiKey.mockReset();
    metadataApiMock.search.mockReset();
    metadataApiMock.apply.mockReset();
  });

  const sleep = (ms: number) =>
    new Promise<void>((resolve) => {
      setTimeout(resolve, ms);
    });

  it("shows missing-key state and allows skipping", async () => {
    const onNext = vi.fn();
    metadataApiMock.getApiKey.mockResolvedValueOnce("   ");

    renderPrompt({ onNext });

    expect(
      await screen.findByText(
        "Нужен RAWG API ключ",
      ),
    ).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", {
        name: "Не сейчас",
      }),
    );
    expect(onNext).toHaveBeenCalled();
  });

  it("treats getApiKey failures as missing-key state", async () => {
    metadataApiMock.getApiKey.mockRejectedValueOnce(new Error("nope"));

    renderPrompt();

    expect(
      await screen.findByText(
        "Нужен RAWG API ключ",
      ),
    ).toBeInTheDocument();
  });

  it("supports manual search via Enter and the search button", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");
    metadataApiMock.search.mockResolvedValue([]);

    const user = userEvent.setup();
    renderPrompt();
    await waitFor(() => expect(metadataApiMock.getApiKey).toHaveBeenCalled());
    await sleep(120); // allow auto-search to kick in

    const input = await screen.findByPlaceholderText(
      "Название игры...",
    );

    await user.clear(input);
    await user.type(input, "Arcadia Redux{enter}");
    await waitFor(() =>
      expect(metadataApiMock.search).toHaveBeenLastCalledWith("Arcadia Redux"),
    );

    await user.clear(input);
    await user.type(input, "Arcadia Button");
    await user.click(screen.getByTitle("Найти"));
    await waitFor(() =>
      expect(metadataApiMock.search).toHaveBeenLastCalledWith("Arcadia Button"),
    );
  });

  it("auto-searches when api key is present", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");
    metadataApiMock.search.mockResolvedValueOnce([
      {
        id: 10,
        name: "Arcadia",
        released: "2020-01-01",
        metacritic: 90,
        background_image: null,
      },
    ]);

    renderPrompt();

    await waitFor(() => expect(metadataApiMock.getApiKey).toHaveBeenCalled());

    await sleep(120);

    await waitFor(() =>
      expect(metadataApiMock.search).toHaveBeenCalledWith("Arcadia"),
    );
    expect(await screen.findByText("Arcadia")).toBeInTheDocument();
  });

  it("notifies on empty search results", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");
    metadataApiMock.search.mockResolvedValueOnce([]);

    renderPrompt();
    await sleep(120);

    await waitFor(() => expect(metadataApiMock.search).toHaveBeenCalled());
    expect(notifyMock).toHaveBeenCalledWith(
      expect.objectContaining({ tone: "info" }),
    );
  });

  it("notifies on search error", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");
    metadataApiMock.search.mockRejectedValueOnce(new Error("boom"));

    renderPrompt();
    await sleep(120);

    await waitFor(() => expect(metadataApiMock.search).toHaveBeenCalled());
    expect(notifyMock).toHaveBeenCalledWith(
      expect.objectContaining({ tone: "error" }),
    );
  });

  it("applies metadata and calls onAfterApply/onNext", async () => {
    const onNext = vi.fn();
    const onAfterApply = vi.fn();
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");
    metadataApiMock.search.mockResolvedValueOnce([
      {
        id: 42,
        name: "Arcadia",
        released: null,
        metacritic: null,
        background_image: null,
      },
    ]);
    metadataApiMock.apply.mockResolvedValueOnce(undefined);

    const user = userEvent.setup();
    renderPrompt({ onNext, onAfterApply });
    await sleep(120);

    await user.click(await screen.findByText("Arcadia"));

    await waitFor(() =>
      expect(metadataApiMock.apply).toHaveBeenCalledWith("g1", 42, false),
    );
    await waitFor(() => expect(onAfterApply).toHaveBeenCalled());
    await waitFor(() => expect(onNext).toHaveBeenCalled());
    await waitFor(() =>
      expect(notifyMock).toHaveBeenCalledWith(
        expect.objectContaining({ tone: "success" }),
      ),
    );
  });

  it("applies with rename option when toggle is enabled", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");
    metadataApiMock.search.mockResolvedValueOnce([
      {
        id: 99,
        name: "Arcadia",
        released: null,
        metacritic: null,
        background_image: null,
      },
    ]);
    metadataApiMock.apply.mockResolvedValueOnce(undefined);

    const user = userEvent.setup();
    renderPrompt();
    await sleep(120);

    const renameSwitch = await screen.findByRole("switch");
    await user.click(renameSwitch);

    await user.click(await screen.findByText("Arcadia"));
    await waitFor(() =>
      expect(metadataApiMock.apply).toHaveBeenCalledWith("g1", 99, true),
    );
  });

  it("handles apply failures without advancing", async () => {
    const onNext = vi.fn();
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");
    metadataApiMock.search.mockResolvedValueOnce([
      {
        id: 7,
        name: "Arcadia",
        released: null,
        metacritic: null,
        background_image: null,
      },
    ]);
    metadataApiMock.apply.mockRejectedValueOnce(new Error("apply-fail"));

    const user = userEvent.setup();
    renderPrompt({ onNext });
    await sleep(120);

    await user.click(await screen.findByText("Arcadia"));

    await waitFor(() => expect(metadataApiMock.apply).toHaveBeenCalled());
    expect(onNext).not.toHaveBeenCalled();
    await waitFor(() =>
      expect(notifyMock).toHaveBeenCalledWith(
        expect.objectContaining({ tone: "error" }),
      ),
    );
  });

  it("does not auto-search when the initial query is blank", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");

    renderPrompt({ game: { id: "g1", name: "   " } });
    await waitFor(() => expect(metadataApiMock.getApiKey).toHaveBeenCalled());

    await sleep(200);

    expect(metadataApiMock.search).not.toHaveBeenCalled();
  });

  it("skips searching when query is empty/whitespace", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");
    metadataApiMock.search.mockResolvedValueOnce([]);

    const user = userEvent.setup();
    renderPrompt();
    await waitFor(() => expect(metadataApiMock.getApiKey).toHaveBeenCalled());
    await sleep(120);

    const input = await screen.findByPlaceholderText(
      "Название игры...",
    );
    await user.clear(input);
    await user.click(screen.getByTitle("Найти"));

    // Only the auto-search should have happened.
    expect(metadataApiMock.search).toHaveBeenCalledTimes(1);
  });

  it("clears the searching state when the latest request completes", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");

    let resolveSearch: ((value: unknown) => void) | undefined;
    metadataApiMock.search.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveSearch = resolve;
        }),
    );

    renderPrompt();
    await waitFor(() => expect(metadataApiMock.getApiKey).toHaveBeenCalled());

    // Auto-search fires.
    await sleep(120);
    const searchButton = screen.getByTitle("Найти");
    expect(searchButton).toBeDisabled();

    if (resolveSearch) resolveSearch([]);
    await waitFor(() => expect(searchButton).not.toBeDisabled());
  });

  it("ignores stale search results and keeps the latest ones", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");

    let resolveA: ((value: unknown) => void) | undefined;
    let resolveB: ((value: unknown) => void) | undefined;

    metadataApiMock.search.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveA = resolve;
        }),
    );
    metadataApiMock.search.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveB = resolve;
        }),
    );

    const user = userEvent.setup();
    renderPrompt();
    await waitFor(() => expect(metadataApiMock.getApiKey).toHaveBeenCalled());

    // Auto-search request A.
    await sleep(120);
    await waitFor(() =>
      expect(metadataApiMock.search).toHaveBeenCalledTimes(1),
    );

    const input = await screen.findByPlaceholderText(
      "Название игры...",
    );
    await user.clear(input);
    // The button is disabled while searching, but Enter still triggers a new request.
    await user.type(input, "B{enter}");

    await waitFor(() =>
      expect(metadataApiMock.search).toHaveBeenLastCalledWith("B"),
    );

    if (resolveB)
      resolveB([
        {
          id: 2,
          name: "B",
          released: null,
          metacritic: null,
          background_image: null,
        },
      ]);
    await waitFor(() => expect(screen.getByText("B")).toBeInTheDocument());

    if (resolveA)
      resolveA([
        {
          id: 1,
          name: "A",
          released: null,
          metacritic: null,
          background_image: null,
        },
      ]);

    // Stale results must not overwrite.
    await waitFor(() =>
      expect(screen.queryByText("A")).not.toBeInTheDocument(),
    );
  });

  it("renders cover images when background_image is present", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");
    metadataApiMock.search.mockResolvedValueOnce([
      {
        id: 10,
        name: "Arcadia",
        released: null,
        metacritic: null,
        background_image: "https://example.invalid/arcadia.jpg",
      },
    ]);

    renderPrompt({ remaining: 1 });
    await sleep(120);

    const img = await screen.findByRole("img", { name: "Arcadia" });
    expect(img).toHaveAttribute("src", "https://example.invalid/arcadia.jpg");
    expect(
      screen.queryByText(/Осталось:/),
    ).not.toBeInTheDocument();
  });

  it("ignores getApiKey completion after unmount (cancelled=true)", async () => {
    let resolveKey: ((value: string) => void) | undefined;
    metadataApiMock.getApiKey.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveKey = resolve;
        }),
    );

    const { unmount } = renderPrompt();
    await waitFor(() => expect(metadataApiMock.getApiKey).toHaveBeenCalled());

    unmount();
    if (resolveKey) resolveKey("rawg-key");

    // Allow the promise handlers to run.
    await sleep(0);
  });

  it("ignores getApiKey rejection after unmount (cancelled=true)", async () => {
    let rejectKey: ((reason?: unknown) => void) | undefined;
    metadataApiMock.getApiKey.mockImplementationOnce(
      () =>
        new Promise((_, reject) => {
          rejectKey = reject;
        }),
    );

    const { unmount } = renderPrompt();
    await waitFor(() => expect(metadataApiMock.getApiKey).toHaveBeenCalled());

    unmount();
    if (rejectKey) rejectKey(new Error("late-fail"));

    await sleep(0);
  });

  it("ignores stale search errors when a newer search finishes", async () => {
    metadataApiMock.getApiKey.mockResolvedValueOnce("rawg-key");

    let rejectA: ((reason?: unknown) => void) | undefined;
    metadataApiMock.search.mockImplementationOnce(
      () =>
        new Promise((_, reject) => {
          rejectA = reject;
        }),
    );
    metadataApiMock.search.mockResolvedValueOnce([
      {
        id: 2,
        name: "B",
        released: null,
        metacritic: null,
        background_image: null,
      },
    ]);

    const user = userEvent.setup();
    renderPrompt();
    await waitFor(() => expect(metadataApiMock.getApiKey).toHaveBeenCalled());

    // Auto-search request A.
    await sleep(120);
    await waitFor(() =>
      expect(metadataApiMock.search).toHaveBeenCalledTimes(1),
    );

    const input = await screen.findByPlaceholderText(
      "Название игры...",
    );

    // Enter triggers a new request even while searching.
    await user.clear(input);
    await user.type(input, "B{enter}");
    await waitFor(() =>
      expect(metadataApiMock.search).toHaveBeenLastCalledWith("B"),
    );

    expect(await screen.findByText("B")).toBeInTheDocument();

    if (rejectA) rejectA(new Error("stale-search-fail"));
    await sleep(0);

    // The stale failure should not surface as an error toast.
    expect(notifyMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ tone: "error" }),
    );
  });

  it("ignores stale apply success when the game changes mid-apply", async () => {
    metadataApiMock.getApiKey.mockResolvedValue("rawg-key");
    metadataApiMock.search
      .mockResolvedValueOnce([
        {
          id: 1,
          name: "Arcadia",
          released: null,
          metacritic: null,
          background_image: null,
        },
      ])
      .mockResolvedValueOnce([
        {
          id: 2,
          name: "B",
          released: null,
          metacritic: null,
          background_image: null,
        },
      ]);

    let resolveApplyA: (() => void) | undefined;
    metadataApiMock.apply.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveApplyA = resolve;
        }),
    );
    metadataApiMock.apply.mockResolvedValueOnce(undefined);

    const onNext = vi.fn();
    const onAfterApply = vi.fn();

    const user = userEvent.setup();
    const { rerender } = render(
      <MemoryRouter>
        <RawgMetadataPrompt
          game={{ id: "g1", name: "Arcadia" }}
          remaining={2}
          onNext={onNext}
          onSkipAll={vi.fn()}
          onAfterApply={onAfterApply}
        />
      </MemoryRouter>,
    );

    await sleep(120);
    await user.click(await screen.findByRole("button", { name: /Arcadia/ }));
    await waitFor(() =>
      expect(metadataApiMock.apply).toHaveBeenCalledWith("g1", 1, false),
    );

    // Switch to a new game while the previous apply is still pending.
    rerender(
      <MemoryRouter>
        <RawgMetadataPrompt
          game={{ id: "g2", name: "B" }}
          remaining={1}
          onNext={onNext}
          onSkipAll={vi.fn()}
          onAfterApply={onAfterApply}
        />
      </MemoryRouter>,
    );

    await sleep(120);
    await user.click(await screen.findByRole("button", { name: /^B/ }));
    await waitFor(() =>
      expect(metadataApiMock.apply).toHaveBeenLastCalledWith("g2", 2, false),
    );
    await waitFor(() => expect(onNext).toHaveBeenCalledTimes(1));

    if (resolveApplyA) resolveApplyA();
    await sleep(0);

    // Stale apply must not advance again.
    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it("ignores stale apply errors when the game changes mid-apply", async () => {
    metadataApiMock.getApiKey.mockResolvedValue("rawg-key");
    metadataApiMock.search
      .mockResolvedValueOnce([
        {
          id: 10,
          name: "Arcadia",
          released: null,
          metacritic: null,
          background_image: null,
        },
      ])
      .mockResolvedValueOnce([
        {
          id: 20,
          name: "B",
          released: null,
          metacritic: null,
          background_image: null,
        },
      ]);

    let rejectApplyA: ((reason?: unknown) => void) | undefined;
    metadataApiMock.apply.mockImplementationOnce(
      () =>
        new Promise<void>((_, reject) => {
          rejectApplyA = reject;
        }),
    );
    metadataApiMock.apply.mockResolvedValueOnce(undefined);

    const onNext = vi.fn();

    const user = userEvent.setup();
    const { rerender } = render(
      <MemoryRouter>
        <RawgMetadataPrompt
          game={{ id: "g1", name: "Arcadia" }}
          remaining={2}
          onNext={onNext}
          onSkipAll={vi.fn()}
        />
      </MemoryRouter>,
    );

    await sleep(120);
    await user.click(await screen.findByRole("button", { name: /Arcadia/ }));
    await waitFor(() =>
      expect(metadataApiMock.apply).toHaveBeenCalledWith("g1", 10, false),
    );

    rerender(
      <MemoryRouter>
        <RawgMetadataPrompt
          game={{ id: "g2", name: "B" }}
          remaining={1}
          onNext={onNext}
          onSkipAll={vi.fn()}
        />
      </MemoryRouter>,
    );

    await sleep(120);
    await user.click(await screen.findByRole("button", { name: /^B/ }));
    await waitFor(() =>
      expect(metadataApiMock.apply).toHaveBeenLastCalledWith("g2", 20, false),
    );
    await waitFor(() => expect(onNext).toHaveBeenCalledTimes(1));

    if (rejectApplyA) rejectApplyA(new Error("stale-apply-fail"));
    await sleep(0);

    // Stale apply error must not surface as an error toast.
    expect(notifyMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ tone: "error" }),
    );
  });
});
