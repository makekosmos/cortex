import { readFileSync } from "node:fs";
import { describe, expect, test } from "bun:test";

const source = readFileSync(new URL("./main.ts", import.meta.url), "utf8");
const hostApiSource = readFileSync(new URL("./host-api.ts", import.meta.url), "utf8");
const preloadSource = readFileSync(new URL("./preload.ts", import.meta.url), "utf8");

describe("Host window safety contracts", () => {
  test("headless mode never shows or focuses windows/dialogs", () => {
    expect(source).toMatch(
      /const\s+headless\s*=\s*process\.env\.KOSMOS_HEADLESS\s*===\s*["']1["']\s*\|\|\s*process\.env\.KOSMOS_TEST_MODE\s*===\s*["']1["']/,
    );
    expect(source).toMatch(/new\s+BrowserWindow\s*\(\s*\{[\s\S]*?show\s*:\s*!headless/);
    expect(source).toMatch(
      /if\s*\(\s*!headless\s*&&\s*app\.isReady\(\)\s*\)\s*dialog\.showErrorBox/,
    );
    expect(source).toMatch(
      /if\s*\(\s*!headless\s*\)\s*\{[\s\S]*?existing\.show\(\);[\s\S]*?existing\.focus\(\);[\s\S]*?\}/,
    );
  });

  test("package windows provide the shared dark native canvas", () => {
    expect(source).toMatch(
      /new\s+BrowserWindow\s*\(\s*\{[\s\S]*?backgroundColor\s*:\s*["']#1d1d1f["']/,
    );
  });

  test("legacy package documents cannot overwrite the native Kosmos window name", () => {
    expect(source).toContain('win.on("page-title-updated"');
    expect(source).toContain("win.setTitle(name);");
  });

  test("Electron wiring delegates terminal cleanup to the ownership authority", () => {
    expect(source).toContain(
      'import { LaunchOwnership, type OwnedLaunch } from "./launch-ownership";',
    );
    expect(source).toContain("const launchId = ownership.take(claim);");
    expect(source).toContain("void revokeLaunch(id, launchId);");
    expect(source).toContain('win.webContents.on("render-process-gone"');
    expect(source).toContain('win.webContents.on("did-fail-load"');
  });

  test("replacement cleans old local accounting before installing the new owner", () => {
    const claim = source.indexOf("const { current: claim, replaced } = ownership.claim(");
    const replacement = source.indexOf("cleanupReplacedLaunch(replaced)", claim);
    const opened = source.indexOf("lifecycle.opened();", claim);
    const installWindow = source.indexOf("windows.set(id, win)", claim);
    expect(claim).toBeGreaterThan(-1);
    expect(opened).toBeGreaterThan(claim);
    expect(opened).toBeLessThan(replacement);
    expect(replacement).toBeGreaterThan(claim);
    expect(replacement).toBeLessThan(installWindow);
    expect(source).toContain("eventSubscribers.delete(replaced.webContentsId)");
    expect(source).toContain("lifecycle.closed();");
  });

  test("shutdown prevents every quit event while its bounded drain is active", () => {
    expect(source).toContain('app.on("before-quit", (event) => {');
    const handler = source.slice(source.indexOf('app.on("before-quit"'));
    expect(handler.indexOf("event.preventDefault();")).toBeLessThan(
      handler.indexOf("if (shutdownDraining) return;"),
    );
    expect(source).toContain("windows.clear();");
    expect(source).toContain("manifests.clear();");
    expect(source).toContain("eventSubscribers.clear();");
    expect(source).toContain('ownership.drain((launchId) => revokeLaunch("shutdown", launchId))');
    expect(source).toContain("setTimeout(resolve, 1_000)");
    expect(source).toContain(".finally(() => app.exit(0))");
  });

  test("a user close during pending navigation is not fatal", () => {
    expect(source).toContain("await win.loadURL(developmentUrl ?? manifest.launch_url);");
    expect(source).toMatch(/if\s*\(win\.isDestroyed\(\)\)\s*return\s*\{/);
    expect(source).toContain('reportFailure("Ресурс приложения недоступен.", initial);');
  });

  test("renderer app navigation uses the existing launch and navigation transport", () => {
    expect(source).toContain('ipcMain.handle("host:app-open"');
    expect(source).toContain("BrowserWindow.fromWebContents(event.sender)");
    expect(source).toContain("parseOpenAppRequest(input)");
    expect(source).toContain("return openApp(");
    expect(source).toContain("parsed.request.id");
    expect(source).toContain("const windowReady = new Map<string, Promise<boolean>>();");
    expect(source).toContain("await sendNavigationWhenReady(windowReady.get(id)");
    expect(source).toContain("const ready = new Promise<boolean>");
    expect(source).toContain('existing.webContents.send("kepler:extension:navigation", route)');
    expect(source).toContain('win.webContents.once("did-finish-load"');
    expect(source).toContain('win.webContents.send("kepler:extension:navigation", route)');
    expect(preloadSource).toContain('"host:app-open"');
    expect(preloadSource).toContain("apps:");
    expect(preloadSource).toContain("navigation:");
  });

  test("live manifest validation resolves without minting a new launch lease", () => {
    expect(source).toContain("await engine.resolveApp(id, launch.version)");
    expect(source).toContain("const result = await engine.launchApp(id);");
  });

  test("single-instance forwarding validates both argument forms", () => {
    expect(source).toMatch(/argument\.startsWith\(["']--open-app=["']\)/);
    expect(source).toMatch(/argument\s*===\s*["']--open-app["']\s*\?\s*argv\[index\s*\+\s*1\]/);
    expect(source).toMatch(/SAFE_ID\.test\(id\)\s*\?\s*id\s*:\s*undefined/);
    expect(source).toMatch(/app\.requestSingleInstanceLock\(\)/);
    expect(source).toMatch(/const\s+id\s*=\s*requested\(argv\)/);
    expect(source).toMatch(
      /await\s+app\.whenReady\(\);\s*await\s+openApp\(id,\s*false,\s*requestedDevelopmentUrl\(argv\)\)/,
    );
  });

  test("only the single-instance owner keeps the warm Host alive without windows", () => {
    const owner = source.search(
      /else\s*\{\s*app\.on\(\s*["']window-all-closed["']\s*,\s*\(\)\s*=>\s*\{\s*}\s*\)/,
    );
    expect(owner).toBeGreaterThan(
      source.indexOf("const singleInstance = app.requestSingleInstanceLock()"),
    );
  });

  test("registers renderer IPC before loading the initial App", () => {
    const launch = source.search(
      /const\s+id\s*=\s*initialOpenAppId;\s*if\s*\(id\)\s*await\s+openApp/,
    );
    expect(source.search(/ipcMain\.on\(\s*["']host:ark-subscribe["']/)).toBeLessThan(launch);
    expect(source.search(/ipcMain\.handle\(\s*["']host:ark-request["']/)).toBeLessThan(launch);
  });

  test("launcher requests are manifest-scoped", () => {
    expect(source).toMatch(/ipcMain\.handle\(\s*["']host:launcher-request["']/);
    for (const operation of [
      "app_index.list_all",
      "app_index.search",
      "app_index.launch",
      "file_index.search",
      "file_index.open",
      "commands.list",
      "commands.invoke",
    ]) {
      expect(source).toMatch(new RegExp(`["']${operation}["']`));
    }
    expect(source).toMatch(/hasLauncherGrant\(manifest\.permissions,\s*operation\)/);
    expect(source).toMatch(/return\s+engine\.launcherRequest\(operation,\s*params\)/);
  });

  test("does not expose an ARK bridge to launcher-only Apps", () => {
    expect(source).toContain(
      '`--kosmos-ark=${isV2Launch(manifest) || hasArkGrant(manifest.permissions) ? "1" : "0"}`',
    );
  });

  test("v2 ARK requests use the launch-scoped Engine boundary", () => {
    const v2 = source.indexOf("if (isV2Launch(manifest))");
    const scoped = source.indexOf("engine.launchArkRequest(", v2);
    const v1 = source.indexOf("return engine.arkRequest(request.operation, params);", scoped);
    expect(v2).toBeGreaterThan(-1);
    expect(scoped).toBeGreaterThan(v2);
    expect(v1).toBeGreaterThan(scoped);
    expect(source.slice(v2, scoped)).not.toContain("manifest.permissions");
  });

  test("launch authority stays in main and uses the signed Engine request contract", () => {
    expect(hostApiSource).toContain("`/v1/apps/launch/${launchId}/ark`");
    expect(hostApiSource).toContain('"X-Kosmos-Launch-Token": brokerToken');
    expect(hostApiSource).toContain("JSON.stringify({ operation, params })");
    const additionalArguments = source.slice(
      source.indexOf("additionalArguments:"),
      source.indexOf("],", source.indexOf("additionalArguments:")) + 2,
    );
    expect(additionalArguments).not.toContain("broker_token");
    expect(additionalArguments).not.toContain("data_api");
    expect(preloadSource).not.toContain("broker_token");
    expect(preloadSource).not.toContain("data_api");
  });

  test("renews v2 launch authority before expiry and clears it on teardown", () => {
    expect(hostApiSource).toContain("`/v1/apps/launch/${launchId}/renew`");
    expect(source).toContain("const renewalTimers = new Map");
    expect(source).toContain("engine.renewLaunch(");
    expect(hostApiSource).toContain("if (untilExpiry <= 1_000) return null;");
    expect(source).toContain("if (win && !win.isDestroyed()) win.close();");
    expect(source).toContain("clearLaunchRenewal(launchId);");
    expect(source).toContain("for (const timer of renewalTimers.values()) clearTimeout(timer);");
  });

  test("v2 authority is revoked before top-level navigation leaves the launch origin", () => {
    expect(source).toContain('win.webContents.on("will-navigate"');
    expect(source).toContain('win.webContents.on("did-start-navigation"');
    expect(source).toContain("event.preventDefault();");
    expect(source).toContain("new URL(developmentUrl ?? manifest.launch_url).origin");
    expect(source).toContain("revokeOnNavigation(url);");
  });

  test("directory picker returns only a launch-scoped opaque grant", () => {
    expect(preloadSource).toContain("pickDirectoryGrant");
    expect(preloadSource).toContain("host:dialogs:pick-directory-grant");
    expect(preloadSource).not.toContain("absoluteRoot");
    expect(preloadSource).not.toContain("filePaths");
    expect(preloadSource).not.toContain("broker_token");

    const picker = source.slice(
      source.indexOf('ipcMain.handle("host:dialogs:pick-directory-grant"'),
    );
    expect(picker).toContain("BrowserWindow.fromWebContents(event.sender)");
    expect(picker).toContain("const appId = [...windows.entries()]");
    expect(picker).toContain(
      "if (!manifest || !isV2Launch(manifest) || !manifest.broker_token) return null;",
    );
    expect(picker).toContain("KOSMOS_TEST_SELECTED_DIRECTORY");
    expect(picker).toContain('properties: ["openDirectory"]');
    expect(picker).toContain("engine.registerDirectoryGrant(");
    expect(picker).toContain("return result.ok ? result.data : null;");
    expect(picker).not.toContain("return selectedDirectory;");
  });

  test("warm-timeout exit enters the bounded before-quit drain", () => {
    const requestExit = source.indexOf("const requestExit = (): void => {");
    const requestExitBody = source.slice(requestExit, source.indexOf("};", requestExit));
    expect(requestExitBody).toContain("app.quit();");
    expect(source).toContain('app.on("before-quit", (event) => {');
    expect(source).toContain(".finally(() => app.exit(0))");
  });
});
