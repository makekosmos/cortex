import { BrowserWindow, session, type WebContents } from "electron";
import type { ArkClient } from "@kosmos/ark";
import { safeHandle } from "./ipc-safe";
import { runLeetCodeLogin } from "./leetcode-auth-flow";

const PARTITION = "persist:leetcode";

type Options = {
  awaitArkReady(): Promise<ArkClient>;
};

async function waitForLeetCodeSession(win: BrowserWindow): Promise<string> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const cookies = win.webContents.session.cookies;
    const onClosed = () => finish(new Error("Вход в LeetCode отменён"));
    const onCookieChanged = () => void check().catch(() => undefined);
    const finish = (error?: Error, credential?: string) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      cookies.removeListener("changed", onCookieChanged);
      win.removeListener("closed", onClosed);
      if (error) reject(error);
      else resolve(credential!);
    };
    const check = async () => {
      if (win.isDestroyed()) return;
      const [authCookies, csrfCookies] = await Promise.all([
        cookies.get({ name: "LEETCODE_SESSION" }),
        cookies.get({ name: "csrftoken" }),
      ]);
      const auth = authCookies[0]?.value;
      const csrf = csrfCookies[0]?.value;
      if (auth && csrf) {
        finish(undefined, JSON.stringify({ session: auth, csrfToken: csrf }));
      }
    };
    const timeout = setTimeout(
      () => finish(new Error("Время входа в LeetCode истекло")),
      5 * 60_000,
    );
    cookies.on("changed", onCookieChanged);
    win.once("closed", onClosed);
    void check();
  });
}

function createLoginWindow(sender: WebContents): BrowserWindow {
  const parent = BrowserWindow.fromWebContents(sender) ?? undefined;
  return new BrowserWindow({
    parent,
    modal: Boolean(parent),
    width: 1080,
    height: 760,
    title: "Вход в LeetCode",
    autoHideMenuBar: true,
    webPreferences: {
      partition: PARTITION,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
}

export function registerLeetCodeIntegrationIpc(options: Options): void {
  safeHandle("kepler:integrations:leetcode:connect", async (event) => {
    return runLeetCodeLogin({
      clearCookies: () =>
        session.fromPartition(PARTITION).clearStorageData({ storages: ["cookies"] }),
      createWindow: () => createLoginWindow(event.sender),
      loadLogin: (win) => win.loadURL("https://leetcode.com/accounts/login/"),
      waitForCredential: waitForLeetCodeSession,
      closeWindow: (win) => {
        if (!win.isDestroyed()) win.close();
      },
      persistCredential: async (credential) => {
        const client = await options.awaitArkReady();
        return client.invokeOperation({
          operation: "integrations.set_credential",
          provider: "leetcode",
          credential,
        });
      },
    });
  });

  safeHandle("kepler:integrations:leetcode:disconnect", async () => {
    const client = await options.awaitArkReady();
    const snapshot = await client.invokeOperation({
      operation: "integrations.clear_credential",
      provider: "leetcode",
    });
    await session.fromPartition(PARTITION).clearStorageData({ storages: ["cookies"] });
    return snapshot;
  });
}
