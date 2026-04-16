import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ToastProvider, useToast } from "@/components/ToastProvider";
import { arrancadorOnMock } from "./bridge";

function ToastHarness({ durationMs = 0 }: { durationMs?: number }) {
  const { notify } = useToast();
  return (
    <div>
      <button
        type="button"
        onClick={() => notify({ title: "Hello", description: "World", durationMs })}
      >
        notify
      </button>
      <button
        type="button"
        onClick={() =>
          notify({ title: "NoTone", tone: undefined as unknown as never })
        }
      >
        no-tone
      </button>
      <button
        type="button"
        onClick={() => notify({ title: "Success", tone: "success" })}
      >
        success-no-desc
      </button>
      <button
        type="button"
        onClick={() => notify({ title: "Warn", tone: "warning" })}
      >
        warn
      </button>
      <button
        type="button"
        onClick={() => notify({ title: "Err", tone: "error" })}
      >
        error
      </button>
    </div>
  );
}

describe("ToastProvider", () => {
  beforeEach(() => {
    arrancadorOnMock.mockReset();
    arrancadorOnMock.mockImplementation(() => () => {});
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders toasts and auto-removes them", async () => {
    const user = userEvent.setup();
    render(
      <ToastProvider>
        <ToastHarness durationMs={200} />
      </ToastProvider>,
    );

    await user.click(screen.getByRole("button", { name: "notify" }));
    expect(await screen.findByText("Hello")).toBeInTheDocument();
    expect(screen.getByText("World")).toBeInTheDocument();

    await new Promise((resolve) => setTimeout(resolve, 250));
    await waitFor(() => {
      expect(screen.queryByText("Hello")).not.toBeInTheDocument();
    });
  });

  it("dismisses toast via button", async () => {
    const user = userEvent.setup();
    render(
      <ToastProvider>
        <ToastHarness durationMs={0} />
      </ToastProvider>,
    );

    await user.click(screen.getByRole("button", { name: "notify" }));
    await user.click(
      await screen.findByRole("button", { name: /закрыть уведомление/i }),
    );

    expect(screen.queryByText("Hello")).not.toBeInTheDocument();
  });

  it("renders different tones and omits description when not provided", async () => {
    const user = userEvent.setup();
    render(
      <ToastProvider>
        <ToastHarness durationMs={0} />
      </ToastProvider>,
    );

    await user.click(screen.getByRole("button", { name: "success-no-desc" }));
    await user.click(screen.getByRole("button", { name: "warn" }));
    await user.click(screen.getByRole("button", { name: "error" }));

    expect(await screen.findByText("Success")).toBeInTheDocument();
    expect(screen.queryByText("World")).not.toBeInTheDocument();
    expect(screen.getByText("Warn")).toBeInTheDocument();
    expect(within(screen.getByRole("alert")).getByText("Err")).toBeInTheDocument();
  });

  it("falls back to the default tone when tone is explicitly undefined", async () => {
    const user = userEvent.setup();
    render(
      <ToastProvider>
        <ToastHarness durationMs={0} />
      </ToastProvider>,
    );

    await user.click(screen.getByRole("button", { name: "no-tone" }));
    const toast = await screen.findByRole("status");
    expect(within(toast).getByText("NoTone")).toBeInTheDocument();
  });

  it("listens for the save-path-missing event and notifies", async () => {
    let handler:
      | ((payload: { game_id: string; game_name: string }) => void)
      | undefined;

    arrancadorOnMock.mockImplementationOnce((eventName, cb) => {
      if (eventName === "game:save-path-missing") {
        handler = cb as (payload: {
          game_id: string;
          game_name: string;
        }) => void;
      }
      return () => {};
    });

    render(
      <ToastProvider>
        <div />
      </ToastProvider>,
    );

    await waitFor(() => expect(arrancadorOnMock).toHaveBeenCalled());
    expect(handler).not.toBeNull();

    if (!handler) throw new Error("missing toast event handler");
    handler({ game_id: "g1", game_name: "Arcadia" });

    expect(await screen.findByText("Сохранения не найдены")).toBeInTheDocument();

    handler(undefined as never);
  });

  it("cleans up the registered bridge listener on unmount", () => {
    const unlisten = vi.fn();
    arrancadorOnMock.mockImplementationOnce(() => unlisten);

    const { unmount } = render(
      <ToastProvider>
        <div />
      </ToastProvider>,
    );

    expect(() => unmount()).not.toThrow();
    expect(unlisten).toHaveBeenCalled();
  });

  it("throws when useToast is used outside provider", () => {
    function BadConsumer() {
      useToast();
      return null;
    }

    expect(() => render(<BadConsumer />)).toThrow(
      "useToast must be used within ToastProvider",
    );
  });
});
