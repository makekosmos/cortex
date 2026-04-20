import fs from "node:fs";
import path from "node:path";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { test, expect } from "@playwright/test";

const appDir = "D:\\Personal\\Hobby\\Coding\\kepler\\apps\\arrancador";
const taskDir =
  "D:\\Personal\\Hobby\\Coding\\kepler\\.agent\\tasks\\2026-04-19-arrancador-playwright-dom-diagnosis";
const rawDir = path.join(taskDir, "raw");
const devUrl = "http://127.0.0.1:5173/";
const bunExe = "C:\\Users\\Kazui\\.bun\\bin\\bun.exe";

let viteProcess: ChildProcessWithoutNullStreams | null = null;

async function waitForServer(url: string, timeoutMs: number) {
  const startedAt = Date.now();

  while (Date.now() - startedAt < timeoutMs) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
    } catch {
      // Server is still booting.
    }

    await new Promise((resolve) => setTimeout(resolve, 500));
  }

  throw new Error(`Timed out waiting for ${url}`);
}

test.describe.configure({ mode: "serial" });

test.beforeAll(async () => {
  fs.mkdirSync(rawDir, { recursive: true });

  viteProcess = spawn(bunExe, ["run", "dev:renderer"], {
    cwd: appDir,
    stdio: ["ignore", "pipe", "pipe"],
  });

  let stdout = "";
  let stderr = "";
  viteProcess.stdout.on("data", (chunk) => {
    stdout += String(chunk);
  });
  viteProcess.stderr.on("data", (chunk) => {
    stderr += String(chunk);
  });

  try {
    await waitForServer(devUrl, 60_000);
  } catch (error) {
    fs.writeFileSync(path.join(rawDir, "vite-dev-renderer.out.log"), stdout, "utf8");
    fs.writeFileSync(path.join(rawDir, "vite-dev-renderer.err.log"), stderr, "utf8");
    throw error;
  }

  fs.writeFileSync(path.join(rawDir, "vite-dev-renderer.out.log"), stdout, "utf8");
  fs.writeFileSync(path.join(rawDir, "vite-dev-renderer.err.log"), stderr, "utf8");
});

test.afterAll(async () => {
  if (!viteProcess || viteProcess.killed) {
    return;
  }

  await new Promise<void>((resolve) => {
    viteProcess?.once("exit", () => resolve());
    viteProcess?.kill();
    setTimeout(() => resolve(), 5_000);
  });
});

test("captures sidebar paint timing and visual state", async ({ page }) => {
  const startedAt = Date.now();

  const consoleMessages: string[] = [];
  page.on("console", (message) => {
    consoleMessages.push(`[${message.type()}] ${message.text()}`);
  });

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => {
    pageErrors.push(error.stack ?? error.message);
  });

  await page.setViewportSize({ width: 1440, height: 960 });
  await page.goto(devUrl, { waitUntil: "domcontentloaded" });
  await page.waitForSelector(".kepler-sidebar-btn", { state: "attached", timeout: 20_000 });
  const sidebarReadyAt = Date.now();

  const screenshotPath = path.join(rawDir, "playwright-sidebar.png");
  await page.screenshot({ path: screenshotPath, fullPage: true });

  const diagnostics = await page.evaluate(() => {
    const firstButton = document.querySelector<HTMLElement>(".kepler-sidebar-btn");
    const sidebar = document.querySelector<HTMLElement>(".kepler-desktop-chrome__sidebar");
    const sidebarBody = document.querySelector<HTMLElement>(".arrancador-sidebar-body");
    const shell = document.querySelector<HTMLElement>(".arrancador-sidebar-shell");
    const rootStyles = getComputedStyle(document.documentElement);
    const paints = performance.getEntriesByType("paint").map((entry) => ({
      name: entry.name,
      startTime: entry.startTime,
    }));
    const navigation = performance.getEntriesByType("navigation")[0] as
      | PerformanceNavigationTiming
      | undefined;

    const serializeRect = (node: Element | null) => {
      if (!(node instanceof HTMLElement)) {
        return null;
      }
      const rect = node.getBoundingClientRect();
      return {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
      };
    };

    return {
      readyState: document.readyState,
      title: document.title,
      textSample: firstButton?.textContent?.trim() ?? null,
      cssVars: {
        sidebarForeground: rootStyles.getPropertyValue("--sidebar-foreground").trim(),
        sidebarBg: rootStyles.getPropertyValue("--sidebar-bg").trim(),
        foreground: rootStyles.getPropertyValue("--foreground").trim(),
        card: rootStyles.getPropertyValue("--card").trim(),
      },
      firstButtonStyles: firstButton
        ? (() => {
            const styles = getComputedStyle(firstButton);
            return {
              color: styles.color,
              backgroundColor: styles.backgroundColor,
              opacity: styles.opacity,
              visibility: styles.visibility,
              display: styles.display,
              pointerEvents: styles.pointerEvents,
            };
          })()
        : null,
      sidebarStyles: sidebar
        ? (() => {
            const styles = getComputedStyle(sidebar);
            return {
              color: styles.color,
              backgroundColor: styles.backgroundColor,
              opacity: styles.opacity,
              visibility: styles.visibility,
              display: styles.display,
            };
          })()
        : null,
      shellStyles: shell
        ? (() => {
            const styles = getComputedStyle(shell);
            return {
              color: styles.color,
              backgroundColor: styles.backgroundColor,
              opacity: styles.opacity,
              visibility: styles.visibility,
              display: styles.display,
            };
          })()
        : null,
      sidebarBodyStyles: sidebarBody
        ? (() => {
            const styles = getComputedStyle(sidebarBody);
            return {
              color: styles.color,
              backgroundColor: styles.backgroundColor,
              opacity: styles.opacity,
              visibility: styles.visibility,
              display: styles.display,
            };
          })()
        : null,
      sidebarRect: serializeRect(sidebar),
      shellRect: serializeRect(shell),
      firstButtonRect: serializeRect(firstButton),
      paints,
      navigation: navigation
        ? {
            responseEnd: navigation.responseEnd,
            domContentLoadedEventEnd: navigation.domContentLoadedEventEnd,
            loadEventEnd: navigation.loadEventEnd,
          }
        : null,
    };
  });

  const report = {
    ...diagnostics,
    timeToSidebarButtonMs: sidebarReadyAt - startedAt,
    consoleMessages,
    pageErrors,
  };

  fs.writeFileSync(
    path.join(rawDir, "playwright-diagnostic.json"),
    JSON.stringify(report, null, 2),
    "utf8",
  );

  expect(diagnostics.textSample).toContain("Библиотека");
  expect(diagnostics.sidebarRect?.width).toBeGreaterThan(150);
});
