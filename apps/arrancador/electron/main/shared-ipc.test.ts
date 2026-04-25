import { describe, expect, it, vi } from "vitest";

const electronMock = vi.hoisted(() => ({
  ipcRendererInvoke: vi.fn(),
  ipcRendererOn: vi.fn(),
  ipcRendererRemoveListener: vi.fn(),
}));

vi.mock("electron", () => ({
  ipcRenderer: {
    invoke: electronMock.ipcRendererInvoke,
    on: electronMock.ipcRendererOn,
    removeListener: electronMock.ipcRendererRemoveListener,
  },
}));

describe("shared IPC registry bridge", () => {
  it("exposes known command methods from the registry", async () => {
    const { createArrancadorBridge } = await import("../shared/ipc");
    const invoke = vi.fn(async (channel: string, payload?: unknown) => ({
      channel,
      payload,
    }));

    const bridge = createArrancadorBridge(invoke);

    expect(typeof bridge.commands.get_all_games).toBe("function");
    expect(typeof bridge.commands.scan_executables_stream).toBe("function");

    await expect(bridge.commands.get_all_games()).resolves.toEqual({
      channel: "get_all_games",
      payload: undefined,
    });
  });

  it("rejects unexpected payloads for no-arg commands", async () => {
    const { createArrancadorBridge } = await import("../shared/ipc");
    const bridge = createArrancadorBridge(vi.fn());

    expect(() => bridge.commands.get_all_games({} as never)).toThrow(
      "expected no payload",
    );
  });

  it("keeps payload validators active for malformed payloads", async () => {
    const { createArrancadorBridge } = await import("../shared/ipc");
    const bridge = createArrancadorBridge(vi.fn());

    expect(() => bridge.commands.set_setting({ key: "theme" } as never)).toThrow(
      "expected string value",
    );
    expect(() =>
      bridge.commands.add_game_process_bindings({
        id: "game-1",
        bindings: [{ match_type: "bad", match_value: "x" }],
      } as never),
    ).toThrow("malformed binding");
  });

  it("allows only http(s) URLs for external shell opens", async () => {
    const { createArrancadorBridge } = await import("../shared/ipc");
    const invoke = vi.fn();
    const bridge = createArrancadorBridge(invoke);

    bridge.commands.shell_open_external({ url: "https://example.com/store" });
    bridge.commands.shell_open_external({ url: "http://example.com/store" });
    expect(() =>
      bridge.commands.shell_open_external({ url: "file:///C:/Windows/System32/calc.exe" }),
    ).toThrow("expected http(s) URL");
    expect(() =>
      bridge.commands.shell_open_external({ url: "javascript:alert(1)" }),
    ).toThrow("expected http(s) URL");
    expect(() =>
      bridge.commands.shell_open_external({ url: "steam://run/123" }),
    ).toThrow("expected http(s) URL");

    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("blocks unknown renderer invoke channels", async () => {
    const { electronRendererInvoke } = await import("../shared/ipc");

    expect(() => electronRendererInvoke("unknown_channel")).toThrow(
      "Blocked IPC invoke",
    );
    expect(electronMock.ipcRendererInvoke).not.toHaveBeenCalled();
  });

  it("blocks unknown event subscriptions", async () => {
    const { createArrancadorBridge } = await import("../shared/ipc");
    const bridge = createArrancadorBridge(vi.fn());

    expect(() => bridge.on("bad:event" as never, vi.fn())).toThrow(
      "Blocked IPC listener",
    );
    expect(electronMock.ipcRendererOn).not.toHaveBeenCalled();
  });
});
