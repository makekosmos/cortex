import { describe, expect, test } from "bun:test";
import { EventEmitter } from "node:events";
import {
  encodeTrustedCookieCredential,
  runIntegrationLogin,
  waitForHuaweiCallback,
} from "./integration-login-credential";

describe("Manager integration login flow", () => {
  test("captures an immediate Huawei redirect before load and removes listeners", async () => {
    const win = new EventEmitter();
    const webContents = new EventEmitter();
    let prevented = false;
    const callback = "hms://redirect_url?code=example&state=expected";
    const fake = Object.assign(win, { webContents, loadURL: async () => {
      webContents.emit("will-redirect", { preventDefault: () => { prevented = true; } }, callback);
    } });
    expect(await waitForHuaweiCallback(fake as never, "https://example.com")).toBe(callback);
    expect(prevented).toBe(true);
    expect(webContents.listenerCount("will-redirect")).toBe(0);
    expect(win.listenerCount("closed")).toBe(0);
  });

  test("Huawei window cancellation rejects and removes navigation listeners", async () => {
    const win = new EventEmitter();
    const webContents = new EventEmitter();
    const fake = Object.assign(win, { webContents, loadURL: async () => { win.emit("closed"); } });
    await expect(waitForHuaweiCallback(fake as never, "https://example.com")).rejects.toThrow("cancelled");
    expect(webContents.listenerCount("will-navigate")).toBe(0);
  });
  test("closes only after Engine persistence and never returns the credential", async () => {
    const events: string[] = [];
    let persisted = "";
    const result = await runIntegrationLogin({
      createWindow: () => (events.push("create"), {}),
      loadLogin: async () => void events.push("load"),
      waitForCredential: async () => {
        events.push("fresh-cookies");
        return encodeTrustedCookieCredential(
          { session: "session", csrf: "csrf", foreign: "drop" },
          ["session", "csrf"],
        )!;
      },
      closeWindow: () => {
        if (events.at(-1) !== "close") events.push("close");
      },
      persistCredential: async (credential) => {
        persisted = credential;
        events.push("persist");
        return { provider: "com.example.demo", hasCredential: true };
      },
    });

    expect(events).toEqual([
      "create",
      "load",
      "fresh-cookies",
      "persist",
      "close",
    ]);
    expect(persisted).toBe('{"session":"session","csrf":"csrf"}');
    expect(JSON.stringify(result)).not.toContain("session");
  });

  test("keeps only contract-approved cookies within the keyring bound", () => {
    expect(
      encodeTrustedCookieCredential(
        { session: "session", csrf: "csrf", analytics: "drop" },
        ["session", "csrf"],
      ),
    ).toBe('{"session":"session","csrf":"csrf"}');
    expect(encodeTrustedCookieCredential({}, ["session"])).toBeNull();
  });

  test("rejects missing or oversized contract cookies", () => {
    expect(encodeTrustedCookieCredential({ session: "" }, ["session"])).toBeNull();
    expect(
      encodeTrustedCookieCredential({ session: "x".repeat(2048) }, ["session"]),
    ).toBeNull();
  });

  test("requires every trusted cookie before persisting a generic session", () => {
    expect(
      encodeTrustedCookieCredential({ session: "ok" }, ["session", "csrf"]),
    ).toBeNull();
    expect(
      encodeTrustedCookieCredential(
        { session: "ok", csrf: "token", foreign: "drop" },
        ["session", "csrf"],
      ),
    ).toBe('{"session":"ok","csrf":"token"}');
  });
});
