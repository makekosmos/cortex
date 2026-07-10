import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import path from "node:path";

test("focus widget routes completion and cancellation through the session lifecycle", async () => {
  const ipcSource = await readFile(path.join(import.meta.dir, "focus-widget-ipc.ts"), "utf8");
  const sessionSource = await readFile(path.join(import.meta.dir, "focus-session.ts"), "utf8");

  expect(ipcSource).toContain('"kepler:focus-widget:pomodoro:complete"');
  expect(ipcSource).toContain("deps.getSessionActions().complete()");
  expect(ipcSource).toContain("deps.getSessionActions().stop()");
  expect(ipcSource).toContain('state.mode === "stopwatch"');
  expect(ipcSource).toContain("Остановить секундомер");
  expect(ipcSource).not.toContain("operation: `pomodoro.${op}`");

  expect(sessionSource).toContain("setFocusWidgetSessionActions({");
  expect(sessionSource).toContain("complete: completeFocusSession");
  expect(sessionSource).toContain("stop: stopFocusSession");
  expect(sessionSource).toContain("showFocusCompletionOverlay(");
});
