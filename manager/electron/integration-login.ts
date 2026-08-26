import {
  BrowserWindow,
  ipcMain,
  session,
  type WebContents,
} from "electron";
import { rpc } from "./engine-client";
import { managerOperations as op } from "../src/manager-api";
import { normalizeIntegrationSnapshot } from "./main-helpers";
import {
  encodeGreatFrontendCredential,
  encodeLeetCodeCredential,
  runIntegrationLogin,
} from "./integration-login-credential";
import { browserPartition } from "./browser-settings";

const LEETCODE_PARTITION = "kosmos-manager-leetcode";
const GREATFRONTEND_PARTITION = "kosmos-manager-greatfrontend";
const GREATFRONTEND_PROGRESS =
  "https://www.greatfrontend.com/profile/progress";

function waitForCredential(
  win: BrowserWindow,
  read: () => Promise<string | null>,
): Promise<string> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const interval = setInterval(() => void check(), 500);
    const timeout = setTimeout(() => finish("timeout"), 5 * 60_000);
    const finish = (reason?: "cancelled" | "timeout", value?: string) => {
      if (settled) return;
      settled = true;
      clearInterval(interval);
      clearTimeout(timeout);
      win.removeListener("closed", onClosed);
      if (reason) reject(new Error(reason));
      else resolve(value!);
    };
    const onClosed = () => finish("cancelled");
    const check = async () => {
      if (settled || win.isDestroyed()) return;
      const value = await read().catch(() => null);
      if (value) finish(undefined, value);
    };
    win.once("closed", onClosed);
    void check();
  });
}

function waitForLeetCodeCredential(win: BrowserWindow): Promise<string> {
  const cookies = win.webContents.session.cookies;
  return waitForCredential(win, async () => {
    const [leetcodeSession, csrfToken] = await Promise.all([
      cookies.get({ url: "https://leetcode.com/", name: "LEETCODE_SESSION" }),
      cookies.get({ url: "https://leetcode.com/", name: "csrftoken" }),
    ]);
    return encodeLeetCodeCredential(
      leetcodeSession[0]?.value,
      csrfToken[0]?.value,
    );
  });
}

function waitForGreatFrontendCredential(win: BrowserWindow): Promise<string> {
  return waitForCredential(win, async () => {
    const cookies = await win.webContents.session.cookies.get({
      url: "https://www.greatfrontend.com/",
    });
    return encodeGreatFrontendCredential(cookies);
  });
}

function createLoginWindow(
  sender: WebContents,
  partition: string,
  title: string,
): BrowserWindow {
  const parent = BrowserWindow.fromWebContents(sender) ?? undefined;
  return new BrowserWindow({
    parent,
    modal: Boolean(parent),
    width: 1080,
    height: 760,
    title,
    autoHideMenuBar: true,
    webPreferences: {
      partition,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
}

type LoginProvider = "leetcode" | "greatfrontend";
const loginConfig = {
  leetcode: {
    label: "LeetCode",
    partition: LEETCODE_PARTITION,
    url: "https://leetcode.com/accounts/login/",
    wait: waitForLeetCodeCredential,
  },
  greatfrontend: {
    label: "GreatFrontEnd",
    partition: GREATFRONTEND_PARTITION,
    url: GREATFRONTEND_PROGRESS,
    wait: waitForGreatFrontendCredential,
  },
} as const;

async function login(
  sender: WebContents,
  provider: LoginProvider,
  getManagerWindow: () => BrowserWindow | null,
  persistBrowserData: () => boolean,
) {
  const config = loginConfig[provider];
  if (
    process.env.KOSMOS_HEADLESS === "1" ||
    process.env.KOSMOS_TEST_MODE === "1"
  )
    return {
      ok: false,
      code: "engine_unavailable",
      message: `Вход через ${config.label} недоступен в этом режиме.`,
    };
  if (BrowserWindow.fromWebContents(sender) !== getManagerWindow())
    return {
      ok: false,
      code: "validation",
      message: "Недопустимый источник входа.",
    };
  try {
    const persistent = persistBrowserData();
    const partition = browserPartition(config.partition, persistent);
    return await runIntegrationLogin({
      createWindow: () =>
        createLoginWindow(sender, partition, `Вход в ${config.label}`),
      loadLogin: (win) => win.loadURL(config.url),
      waitForCredential: config.wait,
      closeWindow: (win) => {
        if (!win.isDestroyed()) win.close();
      },
      persistCredential: async (credential) => {
        const result = await rpc(op.setIntegrationCredential, {
          provider,
          credential,
        });
        return result.ok
          ? { ok: true, data: normalizeIntegrationSnapshot(result.data) }
          : result;
      },
    });
  } catch (cause) {
    const reason = cause instanceof Error ? cause.message : "error";
    return {
      ok: false,
      code: reason === "cancelled" || reason === "timeout" ? "cancelled" : "engine",
      message:
        reason === "timeout"
          ? `Время входа в ${config.label} истекло.`
          : reason === "cancelled"
            ? `Вход в ${config.label} отменён.`
            : `Не удалось завершить вход в ${config.label}.`,
    };
  }
}

export function registerIntegrationLoginHandlers(
  getManagerWindow: () => BrowserWindow | null,
  persistBrowserData: () => boolean,
) {
  ipcMain.handle("manager.loginLeetCode", (event, value) =>
    value === undefined
      ? login(event.sender, "leetcode", getManagerWindow, persistBrowserData)
      : { ok: false, code: "validation", message: "Недопустимые параметры входа." },
  );
  ipcMain.handle("manager.loginGreatFrontend", (event, value) =>
    value === undefined
      ? login(event.sender, "greatfrontend", getManagerWindow, persistBrowserData)
      : { ok: false, code: "validation", message: "Недопустимые параметры входа." },
  );
}

export async function clearIntegrationBrowserData() {
  await Promise.all(
    [LEETCODE_PARTITION, GREATFRONTEND_PARTITION].map((partition) =>
      session
        .fromPartition(browserPartition(partition, true))
        .clearStorageData(),
    ),
  );
}
