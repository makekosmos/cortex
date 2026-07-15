import { BrowserWindow, session } from "electron";
import { randomUUID } from "node:crypto";
import {
  MAX_BOOK_METADATA_HTML_BYTES,
  parseBookMetadataHttpsUrl,
  type BookMetadataPage,
} from "./book-metadata-fetch";
import { createBookMetadataConnectProxy } from "./book-metadata-connect-proxy";

const BROWSER_TIMEOUT_MS = 30_000;
const POLL_INTERVAL_MS = 500;

interface PageSnapshot {
  challenge: boolean;
  finalUrl: string;
  html: string | null;
  htmlLength: number;
  ready: boolean;
  textLength: number;
  tooLarge: boolean;
}

function isAllowedBrowserRequest(rawUrl: string): boolean {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return false;
  }
  if ((url.protocol !== "https:" && url.protocol !== "wss:") || url.username || url.password) {
    return false;
  }
  return true;
}

function isHttpsNavigation(rawUrl: string): boolean {
  try {
    parseBookMetadataHttpsUrl(rawUrl);
    return true;
  } catch {
    return false;
  }
}

function snapshotScript(): string {
  return `(() => {
    const root = document.documentElement;
    if (!root) return null;
    const clone = root.cloneNode(true);
    clone.querySelectorAll("noscript,iframe,object,embed,applet,base,form").forEach((node) => node.remove());
    clone.querySelectorAll("script").forEach((node) => {
      if (node.getAttribute("type")?.trim().toLowerCase() !== "application/ld+json") node.remove();
    });
    clone.querySelectorAll("*").forEach((element) => {
      for (const attribute of Array.from(element.attributes)) {
        if (/^on/i.test(attribute.name) || attribute.name.toLowerCase() === "srcdoc") {
          element.removeAttribute(attribute.name);
        }
      }
    });
    const html = "<!doctype html>\\n" + clone.outerHTML;
    const visibleText = document.body?.innerText ?? "";
    const probe = (document.title + " " + visibleText.slice(0, 2000)).toLowerCase();
    return {
      challenge: /ddos-guard|checking your browser|just a moment|провер(?:ка|яем) браузера/.test(probe),
      finalUrl: location.href,
      html: html.length <= ${MAX_BOOK_METADATA_HTML_BYTES} ? html : null,
      htmlLength: html.length,
      ready: document.readyState === "complete",
      textLength: visibleText.length,
      tooLarge: html.length > ${MAX_BOOK_METADATA_HTML_BYTES},
    };
  })()`;
}

async function waitForRenderedPage(win: BrowserWindow): Promise<BookMetadataPage> {
  const deadline = Date.now() + BROWSER_TIMEOUT_MS;
  let lastKey = "";
  let stableSamples = 0;

  while (Date.now() < deadline) {
    if (win.isDestroyed() || win.webContents.isDestroyed()) {
      throw new Error("Не удалось открыть страницу");
    }
    try {
      const snapshot = (await win.webContents.executeJavaScriptInIsolatedWorld(1001, [
        { code: snapshotScript() },
      ])) as PageSnapshot | null;
      if (snapshot?.tooLarge) throw new Error("Страница слишком большая");
      if (snapshot?.ready && !snapshot.challenge && snapshot.textLength >= 100 && snapshot.html) {
        parseBookMetadataHttpsUrl(snapshot.finalUrl);
        const key = `${snapshot.finalUrl}:${snapshot.htmlLength}`;
        stableSamples = key === lastKey ? stableSamples + 1 : 1;
        lastKey = key;
        if (stableSamples >= 2) {
          if (Buffer.byteLength(snapshot.html, "utf8") > MAX_BOOK_METADATA_HTML_BYTES) {
            throw new Error("Страница слишком большая");
          }
          return { finalUrl: snapshot.finalUrl, html: snapshot.html };
        }
      } else {
        stableSamples = 0;
        lastKey = "";
      }
    } catch (error) {
      if (error instanceof Error && error.message === "Страница слишком большая") throw error;
      // Navigation can replace the execution context between challenge pages.
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
  }
  throw new Error("Страница не загрузилась вовремя");
}

export async function fetchBookMetadataPageInBrowser(source: string): Promise<BookMetadataPage> {
  const url = parseBookMetadataHttpsUrl(source);
  const connectProxy = await createBookMetadataConnectProxy();
  const isolatedSession = session.fromPartition(`book-metadata-${randomUUID()}`, { cache: false });
  const denyDownload = (event: Electron.Event): void => event.preventDefault();
  let win: BrowserWindow | undefined;

  try {
    await isolatedSession.setProxy({
      mode: "fixed_servers",
      ...connectProxy.config,
    });

    isolatedSession.setPermissionCheckHandler(() => false);
    isolatedSession.setPermissionRequestHandler((_webContents, _permission, callback) =>
      callback(false),
    );
    isolatedSession.on("will-download", denyDownload);
    isolatedSession.webRequest.onBeforeRequest({ urls: ["<all_urls>"] }, (details, callback) => {
      callback({ cancel: !isAllowedBrowserRequest(details.url) });
    });

    const userAgent = isolatedSession
      .getUserAgent()
      .replace(/\sElectron\/\S+/g, "")
      .replace(/\sKosmos\/\S+/g, "");
    isolatedSession.setUserAgent(userAgent, "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7");

    win = new BrowserWindow({
      show: false,
      skipTaskbar: true,
      webPreferences: {
        session: isolatedSession,
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
        webviewTag: false,
        webSecurity: true,
        allowRunningInsecureContent: false,
        backgroundThrottling: false,
        spellcheck: false,
      },
    });
    win.webContents.setAudioMuted(true);
    win.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
    win.webContents.on("will-attach-webview", (event) => event.preventDefault());
    win.webContents.on("will-navigate", (event, navigationUrl) => {
      if (!isHttpsNavigation(navigationUrl)) event.preventDefault();
    });
    win.webContents.on("will-redirect", (event, navigationUrl) => {
      if (!isHttpsNavigation(navigationUrl)) event.preventDefault();
    });

    void win.loadURL(url.href).catch(() => undefined);
    return await waitForRenderedPage(win);
  } finally {
    if (win && !win.isDestroyed()) win.destroy();
    isolatedSession.webRequest.onBeforeRequest(null);
    isolatedSession.removeListener("will-download", denyDownload);
    isolatedSession.setPermissionCheckHandler(null);
    isolatedSession.setPermissionRequestHandler(null);
    await Promise.allSettled([
      isolatedSession.clearCache(),
      isolatedSession.clearStorageData(),
      isolatedSession.closeAllConnections(),
      connectProxy.close(),
    ]);
  }
}
