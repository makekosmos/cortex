import { describe, expect, it, vi } from "vitest";
import { createMainWindow } from "./windows";

const mocks = vi.hoisted(() => {
  const instances: unknown[] = [];
  class MockWindow {
    webContents = {
      on: vi.fn(),
      setWindowOpenHandler: vi.fn(),
    };
    isDestroyed = vi.fn(() => false);
    isVisible = vi.fn(() => true);
    isMinimized = vi.fn(() => false);
    setTitleBarOverlay = vi.fn();
    show = vi.fn();
    focus = vi.fn();
    restore = vi.fn();
    hide = vi.fn();
    loadURL = vi.fn(async () => undefined);
    loadFile = vi.fn(async () => undefined);
    once = vi.fn();

    constructor(public options: unknown) {
      instances.push(this);
    }
  }

  return {
    BrowserWindow: vi.fn(function BrowserWindow(options: unknown) {
      return new MockWindow(options);
    }),
    instances,
    nativeTheme: {
      shouldUseDarkColors: false,
      on: vi.fn(),
      off: vi.fn(),
    },
    nativeImage: {
      createFromDataURL: vi.fn(),
      createFromPath: vi.fn(),
    },
    shell: {
      openExternal: vi.fn(),
    },
  };
});

vi.mock("electron", () => ({
  BrowserWindow: mocks.BrowserWindow,
  nativeImage: mocks.nativeImage,
  nativeTheme: mocks.nativeTheme,
  shell: mocks.shell,
}));

describe("createMainWindow", () => {
  it("keeps the renderer isolated and sandboxed", () => {
    createMainWindow({
      appPath: "D:\\Apps\\arrancador",
      preloadPath: "D:\\Apps\\arrancador\\out\\preload\\index.js",
      rendererUrl: "http://127.0.0.1:5173",
      icon: {} as never,
      title: "Arrancador",
      width: 1200,
      height: 800,
      minWidth: 900,
      minHeight: 600,
    });

    expect(mocks.BrowserWindow).toHaveBeenCalledOnce();
    expect(mocks.BrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({
        webPreferences: expect.objectContaining({
          contextIsolation: true,
          nodeIntegration: false,
          sandbox: true,
        }),
      }),
    );
  });

  it("opens only parsed http(s) URLs from renderer window-open attempts", () => {
    createMainWindow({
      appPath: "D:\\Apps\\arrancador",
      preloadPath: "D:\\Apps\\arrancador\\out\\preload\\index.js",
      rendererUrl: "http://127.0.0.1:5173",
      icon: {} as never,
      title: "Arrancador",
      width: 1200,
      height: 800,
      minWidth: 900,
      minHeight: 600,
    });

    const window = mocks.instances.at(-1) as {
      webContents: {
        setWindowOpenHandler: ReturnType<typeof vi.fn>;
      };
    };
    const handler = window.webContents.setWindowOpenHandler.mock.calls[0][0] as (details: {
      url: string;
    }) => { action: "deny" };

    expect(handler({ url: "https://example.com/store" })).toEqual({ action: "deny" });
    expect(handler({ url: "file:///C:/Windows/System32/calc.exe" })).toEqual({
      action: "deny",
    });
    expect(handler({ url: "javascript:alert(1)" })).toEqual({ action: "deny" });

    expect(mocks.shell.openExternal).toHaveBeenCalledTimes(1);
    expect(mocks.shell.openExternal).toHaveBeenCalledWith("https://example.com/store");
  });
});
