import { act, render, screen, waitFor } from "@testing-library/react";
import { useIsMobile } from "@/hooks/use-mobile";

function Harness() {
  const isMobile = useIsMobile();
  return <div data-testid="value">{isMobile ? "mobile" : "desktop"}</div>;
}

describe("useIsMobile", () => {
  it("returns mobile for innerWidth < breakpoint and updates on media change", async () => {
    let mql: MediaQueryList | null = null;

    window.matchMedia = vi.fn().mockImplementation((query: string) => {
      const listeners = new Set<(event: MediaQueryListEvent) => void>();
      mql = {
        matches: false,
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: (
          _type: string,
          listener: EventListenerOrEventListenerObject | null,
        ) => {
          if (typeof listener === "function") {
            listeners.add(listener as (event: MediaQueryListEvent) => void);
          }
        },
        removeEventListener: (
          _type: string,
          listener: EventListenerOrEventListenerObject | null,
        ) => {
          if (typeof listener === "function") {
            listeners.delete(listener as (event: MediaQueryListEvent) => void);
          }
        },
        dispatchEvent: (event: Event) => {
          listeners.forEach((listener) => {
            listener(event as MediaQueryListEvent);
          });
          return true;
        },
      };
      return mql;
    });

    Object.defineProperty(window, "innerWidth", { value: 0, writable: true });
    render(<Harness />);
    expect(screen.getByTestId("value")).toHaveTextContent("mobile");

    Object.defineProperty(window, "innerWidth", { value: 100, writable: true });
    await waitFor(() => expect(mql).not.toBeNull());
    act(() => {
      mql?.dispatchEvent(new Event("change") as MediaQueryListEvent);
    });
    expect(screen.getByTestId("value")).toHaveTextContent("desktop");
  });
});
