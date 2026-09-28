import type { IpcMainInvokeEvent } from "electron";
import { expect, mock, test } from "../test-support/node-test.mjs";

type IpcHandler = (event: IpcMainInvokeEvent, url: string) => Promise<void>;
const handlers = new Map<string, IpcHandler>();
const opened: string[] = [];

mock.module("electron", () => ({
  ipcMain: {
    handle: (channel: string, handler: IpcHandler) => {
      handlers.set(channel, handler);
    },
  },
  shell: {
    openExternal: async (url: string) => {
      opened.push(url);
    },
  },
}));

const { registerMainShellIpc } = await import("./main-shell-ipc");

// SAFETY: the openExternal handler ignores the event; a stub stands in for the
// real IpcMainInvokeEvent that ipcMain.handle supplies.
const invokeEvent = {} as IpcMainInvokeEvent;

function openExternalHandler(): IpcHandler {
  registerMainShellIpc({
    awaitArkReady: async () => {
      throw new Error("ark not needed for openExternal");
    },
    getArkClient: () => null,
  });
  const handler = handlers.get("kepler:shell:openExternal");
  if (!handler) throw new Error("kepler:shell:openExternal not registered");
  return handler;
}

test("openExternal rejects non-network URL schemes", async () => {
  const handler = openExternalHandler();
  opened.length = 0;
  const dangerous = [
    "file:///C:/Windows/System32/calc.exe",
    "file://///evil.example/share/payload.exe",
    "\\\\evil.example\\share\\payload.exe",
    "javascript:alert(1)",
    "vbscript:msgbox(1)",
    "data:text/html,<script>alert(1)</script>",
    "ms-settings:home",
    "not a url",
    "",
    "   ",
  ];
  for (const url of dangerous) {
    let rejected = false;
    try {
      await handler(invokeEvent, url);
    } catch {
      rejected = true;
    }
    expect(rejected).toBe(true);
  }
  expect(opened).toEqual([]);
});

test("openExternal forwards browser-safe URL schemes", async () => {
  const handler = openExternalHandler();
  opened.length = 0;
  for (const url of ["https://example.com/page", "http://example.com", "mailto:a@b.c"]) {
    await handler(invokeEvent, url);
  }
  expect(opened).toEqual(["https://example.com/page", "http://example.com", "mailto:a@b.c"]);
});
