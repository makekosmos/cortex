import { setTimeout as delay } from "node:timers/promises";
import { BrowserWindow, screen } from "electron";

export interface WindowMoveBenchmarkInput {
  windowKind?: "launcher" | "settings" | "extension" | "flatTest";
  steps?: number;
  intervalMs?: number;
  acrossDisplays?: boolean;
}

export interface WindowMoveBenchmarkResult {
  total_ms: number;
  intervals_p50_ms: number;
  intervals_p95_ms: number;
  intervals_max_ms: number;
  frames_over_24ms: number;
  frames_over_33ms: number;
  frames_over_50ms: number;
}

function percentile(values: number[], p: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.floor(((sorted.length - 1) * p) / 100);
  return sorted[index] ?? 0;
}

function findBenchmarkWindow(kind: WindowMoveBenchmarkInput["windowKind"]): BrowserWindow | null {
  const windows = BrowserWindow.getAllWindows().filter((win) => !win.isDestroyed());
  if (kind === "launcher") {
    return (
      windows.find((win) => win.webContents.getURL().includes("launcher")) ?? windows[0] ?? null
    );
  }
  if (kind === "settings") {
    return (
      windows.find((win) => win.getTitle().toLowerCase().includes("settings")) ??
      windows.find((win) => win.webContents.getURL().includes("settings")) ??
      null
    );
  }
  if (kind === "extension") {
    return windows.find((win) => win.webContents.getURL().includes("extensions")) ?? null;
  }
  return windows[0] ?? null;
}

function createFlatBenchmarkWindow(): BrowserWindow {
  const headless = process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
  const win = new BrowserWindow({
    width: 720,
    height: 460,
    show: !headless,
    skipTaskbar: headless,
    frame: false,
    transparent: false,
    webPreferences: {
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  void win.loadURL("data:text/html,<html><body></body></html>");
  return win;
}

export async function runWindowMoveBenchmark(
  input: WindowMoveBenchmarkInput = {},
): Promise<WindowMoveBenchmarkResult> {
  const steps = Math.max(1, Math.min(5_000, Math.floor(input.steps ?? 600)));
  const intervalMs = Math.max(1, Math.min(1_000, Math.floor(input.intervalMs ?? 16)));
  const kind = input.windowKind ?? "launcher";
  const ownsWindow = kind === "flatTest";
  const win = ownsWindow ? createFlatBenchmarkWindow() : findBenchmarkWindow(kind);
  if (!win || win.isDestroyed()) {
    throw new Error(`No benchmark window found for kind '${kind}'`);
  }

  const original = win.getBounds();
  const displays = screen.getAllDisplays();
  const targetDisplay =
    input.acrossDisplays && displays.length > 1
      ? displays.find((display) => display.id !== screen.getDisplayMatching(original).id)
      : null;
  const area = (targetDisplay ?? screen.getDisplayMatching(original)).workArea;
  const maxX = Math.max(area.x, area.x + area.width - original.width);
  const maxY = Math.max(area.y, area.y + area.height - original.height);
  const minX = area.x;
  const minY = area.y;
  const intervals: number[] = [];
  const started = Date.now();
  let last = started;

  try {
    for (let index = 0; index < steps; index += 1) {
      const phase = steps <= 1 ? 0 : index / (steps - 1);
      const x = Math.round(minX + (maxX - minX) * phase);
      const y = Math.round(minY + (maxY - minY) * ((index % 2 === 0 ? phase : 1 - phase) * 0.35));
      win.setBounds({ ...original, x, y }, false);
      await delay(intervalMs);
      const now = Date.now();
      intervals.push(now - last);
      last = now;
    }
  } finally {
    if (!win.isDestroyed()) {
      if (ownsWindow) win.destroy();
      else win.setBounds(original, false);
    }
  }

  return {
    total_ms: Date.now() - started,
    intervals_p50_ms: percentile(intervals, 50),
    intervals_p95_ms: percentile(intervals, 95),
    intervals_max_ms: intervals.length > 0 ? Math.max(...intervals) : 0,
    frames_over_24ms: intervals.filter((value) => value > 24).length,
    frames_over_33ms: intervals.filter((value) => value > 33).length,
    frames_over_50ms: intervals.filter((value) => value > 50).length,
  };
}
