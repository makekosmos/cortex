import { describe, expect, it, vi } from "vitest";
import { registerShellScanIpcHandlers } from "./shell-scan-handlers";

const mocks = vi.hoisted(() => ({
  handle: vi.fn(),
  openExternal: vi.fn(async () => undefined),
  openPath: vi.fn(async () => ""),
}));

vi.mock("electron", () => ({
  app: {
    getLoginItemSettings: vi.fn(() => ({ openAtLogin: false })),
    setLoginItemSettings: vi.fn(),
  },
  BrowserWindow: {
    getFocusedWindow: vi.fn(() => null),
    getAllWindows: vi.fn(() => []),
  },
  dialog: {
    showOpenDialog: vi.fn(async () => ({ canceled: true, filePaths: [] })),
  },
  ipcMain: {
    handle: mocks.handle,
  },
  shell: {
    openExternal: mocks.openExternal,
    openPath: mocks.openPath,
  },
}));

vi.mock("../services/scan", () => ({
  createScanCancellation: vi.fn(() => ({ cancel: vi.fn(), signal: new AbortController().signal })),
  getRunningProcesses: vi.fn(async () => []),
  scanExecutablesStream: vi.fn(async () => 0),
}));

describe("registerShellScanIpcHandlers", () => {
  it("opens only http(s) external URLs from IPC", async () => {
    const withRuntime = vi.fn((handler) => async (_event: unknown, payload: unknown) =>
      handler({ currentScan: null, services: { achievements: { recordAchievementEvent: vi.fn() } } }, payload),
    );

    registerShellScanIpcHandlers({
      withRuntime,
      emitRendererEvent: vi.fn(),
    });

    const handlers = new Map(
      mocks.handle.mock.calls.map(([channel, handler]) => [channel, handler]),
    );
    const openExternal = handlers.get("shell_open_external");

    await expect(openExternal?.(null, { url: "https://example.com/store" })).resolves.toBeUndefined();
    await expect(openExternal?.(null, { url: "http://example.com/store" })).resolves.toBeUndefined();
    await expect(openExternal?.(null, { url: "file:///C:/Windows/System32/calc.exe" })).rejects.toThrow(
      "Blocked external URL protocol",
    );
    await expect(openExternal?.(null, { url: "javascript:alert(1)" })).rejects.toThrow(
      "Blocked external URL protocol",
    );
    await expect(openExternal?.(null, { url: "steam://run/123" })).rejects.toThrow(
      "Blocked external URL protocol",
    );

    expect(mocks.openExternal).toHaveBeenCalledTimes(2);
    expect(mocks.openExternal).toHaveBeenNthCalledWith(1, "https://example.com/store");
    expect(mocks.openExternal).toHaveBeenNthCalledWith(2, "http://example.com/store");
  });
});
