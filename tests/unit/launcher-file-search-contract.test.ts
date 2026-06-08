import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const launcherSource = readFileSync("platform/desktop/src/views/LauncherView.vue", "utf8");
const mainSource = readFileSync("platform/desktop/electron/main.ts", "utf8");
const preloadSource = readFileSync("platform/desktop/electron/preload.ts", "utf8");
const ipcTypesSource = readFileSync("platform/desktop/shared/ipc-types.ts", "utf8");

describe("launcher file search contract", () => {
  test("file search does not self-reschedule after a result", () => {
    // Regression: 2026-06-08. Hidden launcher used to poll file_index.search forever.
    expect(launcherSource).not.toContain("FILE_SEARCH_REFRESH_MS");
    expect(launcherSource).not.toContain("scheduleFileSearch(text, run, FILE_SEARCH_REFRESH_MS)");
  });

  test("hidden launcher cancels pending file search work", () => {
    expect(mainSource).toContain('webContents.send("kepler:window:hide")');
    expect(preloadSource).toContain("onHide: (listener)");
    expect(preloadSource).toContain('ipcRenderer.on("kepler:window:hide"');
    expect(ipcTypesSource).toContain("onHide(listener: () => void): () => void;");
    expect(launcherSource).toContain("cancelFileSearch();");
    expect(launcherSource).toContain("offHide = window.kepler.window.onHide");
  });

  test("launcher only disables background throttling on macOS occlusion workaround path", () => {
    expect(mainSource).toContain('backgroundThrottling: process.platform !== "darwin"');
    expect(mainSource).not.toContain("backgroundThrottling: false,");
  });
});
