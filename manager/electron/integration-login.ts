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
  encodeTrustedCookieCredential,
  runIntegrationLogin,
} from "./integration-login-credential";
import { browserPartition } from "./browser-settings";

const GENERIC_INTEGRATION_PARTITION = "kosmos-manager-generic";
const VALID_PROVIDER = /^[a-z0-9][a-z0-9._-]{0,127}$/;

type TrustedLoginContract = {
  provider: string;
  label: string;
  startUrl: string;
  completionUrl: string;
  allowedCookieNames: string[];
  secretSetting: string;
};

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

function readLoginInput(value: unknown): { provider: string } | null {
  if (!value || typeof value !== "object") return null;
  const input = value as Record<string, unknown>;
  if (Object.keys(input).length !== 1) return null;
  const provider = input.provider;
  return typeof provider === "string" && VALID_PROVIDER.test(provider)
    ? { provider }
    : null;
}

function parseTrustedLoginContract(value: unknown): TrustedLoginContract | null {
  if (!value || typeof value !== "object") return null;
  const input = value as Record<string, unknown>;
  const provider = typeof input.provider === "string" ? input.provider : "";
  const login = input.login;
  if (!login || typeof login !== "object") return null;
  const raw = login as Record<string, unknown>;
  const label = typeof input.label === "string" ? input.label.trim() : "";
  const startUrl = typeof raw.startUrl === "string" ? raw.startUrl : "";
  const completionUrl = typeof raw.completionUrl === "string" ? raw.completionUrl : "";
  const allowedCookieNames = Array.isArray(raw.allowedCookieNames)
    ? raw.allowedCookieNames.filter(
        (name): name is string => typeof name === "string" && name.length > 0,
      )
    : [];
  const secretSetting = typeof raw.secretSetting === "string" ? raw.secretSetting : "";
  try {
    const start = new URL(startUrl);
    const completion = new URL(completionUrl);
    if (!label || start.protocol !== "https:" || completion.protocol !== "https:" || !secretSetting || allowedCookieNames.length === 0) return null;
  } catch {
    return null;
  }
  return {
    provider,
    label,
    startUrl,
    completionUrl,
    allowedCookieNames,
    secretSetting,
  };
}

function sameLoginPage(current: string, expected: string): boolean {
  try {
    const actual = new URL(current);
    const target = new URL(expected);
    return actual.protocol === target.protocol && actual.host === target.host && actual.pathname === target.pathname;
  } catch {
    return false;
  }
}

function waitForTrustedCredential(win: BrowserWindow, contract: TrustedLoginContract): Promise<string> {
  return waitForCredential(win, async () => {
    if (!sameLoginPage(win.webContents.getURL(), contract.completionUrl)) return null;
    const origins = [...new Set([new URL(contract.startUrl).origin, new URL(contract.completionUrl).origin])];
    const values: Record<string, string> = {};
    for (const origin of origins) {
      const cookies = await Promise.all(contract.allowedCookieNames.map(async (name) => {
        const found = await win.webContents.session.cookies.get({ url: `${origin}/`, name });
        return [name, found[0]?.value] as const;
      }));
      for (const [name, value] of cookies) if (value) values[name] = value;
    }
    return encodeTrustedCookieCredential(values, contract.allowedCookieNames);
  });
}

async function login(
  sender: WebContents,
  provider: string,
  getManagerWindow: () => BrowserWindow | null,
  persistBrowserData: () => boolean,
) {
  if (
    process.env.KOSMOS_HEADLESS === "1" ||
    process.env.KOSMOS_TEST_MODE === "1"
  )
    return {
      ok: false,
      code: "engine_unavailable",
      message: `Вход через ${provider} недоступен в этом режиме.`,
    };
  if (BrowserWindow.fromWebContents(sender) !== getManagerWindow())
    return {
      ok: false,
      code: "validation",
      message: "Недопустимый источник входа.",
    };
  try {
    const contractResult = await rpc(op.integrationLoginContract, { provider });
    if (!contractResult.ok) return contractResult;
    const contract = parseTrustedLoginContract(contractResult.data);
    if (!contract || contract.provider !== provider)
      return {
        ok: false,
        code: "validation" as const,
        message: "Интеграция не содержит доверенного сценария входа.",
      };
    const persistent = persistBrowserData();
    const partition = browserPartition(GENERIC_INTEGRATION_PARTITION, persistent);
    return await runIntegrationLogin({
      createWindow: () =>
        createLoginWindow(
          sender,
          partition,
          `Вход в ${contract.label}`,
        ),
      loadLogin: (win) => win.loadURL(contract.startUrl),
      waitForCredential: (win) => waitForTrustedCredential(win, contract),
      closeWindow: (win) => {
        if (!win.isDestroyed()) win.close();
      },
      persistCredential: async (credential) => {
        const result = await rpc(op.setIntegrationCredential, {
          provider,
          ...(contract ? { setting: contract.secretSetting } : {}),
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
          ? `Время входа в ${provider} истекло.`
          : reason === "cancelled"
            ? `Вход в ${provider} отменён.`
            : `Не удалось завершить вход в ${provider}.`,
    };
  }
}

export function registerIntegrationLoginHandlers(
  getManagerWindow: () => BrowserWindow | null,
  persistBrowserData: () => boolean,
) {
  ipcMain.handle("manager.loginIntegration", (event, value) => {
    const input = readLoginInput(value);
    return input
      ? login(
          event.sender,
          input.provider,
          getManagerWindow,
          persistBrowserData,
        )
      : {
          ok: false,
          code: "validation",
          message: "Недопустимые параметры входа.",
        };
  });
}

export async function clearIntegrationBrowserData() {
  await Promise.all(
    [GENERIC_INTEGRATION_PARTITION].map((partition) =>
      session
        .fromPartition(browserPartition(partition, true))
        .clearStorageData(),
    ),
  );
}
