import "@testing-library/jest-dom/vitest";
import { resetArrancadorBridge } from "./bridge";

// Electron bridge and legacy renderer hooks are stubbed here for unit tests.
type DragDropHandler = (event: unknown) => void;

const dragDropHandlers: DragDropHandler[] = [];
(
  globalThis as unknown as { __arrancadorDragDropHandlers?: DragDropHandler[] }
).__arrancadorDragDropHandlers = dragDropHandlers;

resetArrancadorBridge();

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: vi.fn().mockImplementation(async (cb: DragDropHandler) => {
      dragDropHandlers.push(cb);
      return () => {
        const idx = dragDropHandlers.indexOf(cb);
        if (idx >= 0) dragDropHandlers.splice(idx, 1);
      };
    }),
  }),
}));

if (!("ResizeObserver" in globalThis)) {
  class ResizeObserverMock {
    observe() {}
    unobserve() {}
    disconnect() {}
  }

  globalThis.ResizeObserver = ResizeObserverMock;
}

if (!window.matchMedia) {
  window.matchMedia = (query: string): MediaQueryList => {
    const listeners = new Set<(event: MediaQueryListEvent) => void>();

    const mql: MediaQueryList = {
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: (
        _type: string,
        listener: EventListenerOrEventListenerObject | null,
      ) => {
        if (typeof listener === "function") {
          listeners.add(listener as (event: MediaQueryListEvent) => void);
        } else if (listener && "handleEvent" in listener) {
          listeners.add(((event: MediaQueryListEvent) =>
            listener.handleEvent(event)) as (
            event: MediaQueryListEvent,
          ) => void);
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
  };
}

Object.defineProperty(navigator, "clipboard", {
  configurable: true,
  value: {
    writeText: vi.fn().mockResolvedValue(undefined),
  },
});
