import type { BrowserWindow, Event } from "electron";

export function encodeTrustedCookieCredential(
  values: Record<string, string | undefined>,
  requiredNames: string[],
): string | null {
  if (
    requiredNames.length === 0 ||
    requiredNames.some((name) => !values[name])
  )
    return null;
  const credential = JSON.stringify(
    Object.fromEntries(requiredNames.map((name) => [name, values[name]])),
  );
  return credential.length <= 2048 ? credential : null;
}

interface IntegrationLoginFlow<TWindow, TResult> {
  createWindow(): TWindow;
  loadLogin(window: TWindow): Promise<void>;
  waitForCredential(window: TWindow): Promise<string>;
  closeWindow(window: TWindow): void;
  persistCredential(credential: string): Promise<TResult>;
}

export async function runIntegrationLogin<TWindow, TResult>(
  flow: IntegrationLoginFlow<TWindow, TResult>,
): Promise<TResult> {
  const window = flow.createWindow();
  let closed = false;
  const closeWindow = () => {
    if (closed) return;
    closed = true;
    flow.closeWindow(window);
  };
  try {
    await flow.loadLogin(window);
    const credential = await flow.waitForCredential(window);
    const result = await flow.persistCredential(credential);
    closeWindow();
    return result;
  } finally {
    closeWindow();
  }
}

export function waitForHuaweiCallback(win: BrowserWindow, startUrl: string): Promise<string> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (value?: string, reason = "cancelled") => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      win.removeListener("closed", onClosed);
      win.webContents.removeListener("will-redirect", onNavigate);
      win.webContents.removeListener("will-navigate", onNavigate);
      if (value) resolve(value);
      else reject(new Error(reason));
    };
    const onClosed = () => finish();
    const onNavigate = (event: Event, url: string) => {
      if (!url.startsWith("hms:")) return;
      event.preventDefault();
      try {
        const target = new URL(url);
        if (target.host === "redirect_url" && target.pathname === "") finish(url);
        else finish(undefined, "invalid-callback");
      } catch { finish(undefined, "invalid-callback"); }
    };
    const timeout = setTimeout(() => finish(undefined, "timeout"), 5 * 60_000);
    win.once("closed", onClosed);
    win.webContents.on("will-redirect", onNavigate);
    win.webContents.on("will-navigate", onNavigate);
    // Attach before navigation: an existing Huawei browser session may redirect immediately.
    void win.loadURL(startUrl).catch(() => finish(undefined, "navigation-failed"));
  });
}
