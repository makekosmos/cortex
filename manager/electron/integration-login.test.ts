import { describe, expect, test } from "bun:test";
import {
  encodeCookieCredential,
  encodeLeetCodeCredential,
  runIntegrationLogin,
} from "./integration-login-credential";

describe("Manager integration login flow", () => {
  test("closes before Engine persistence and never returns the credential", async () => {
    const events: string[] = [];
    let persisted = "";
    const result = await runIntegrationLogin({
      clearCookies: async () => void events.push("clear"),
      createWindow: () => (events.push("create"), {}),
      loadLogin: async () => void events.push("load"),
      waitForCredential: async () => {
        events.push("fresh-cookies");
        return encodeLeetCodeCredential("session", "csrf")!;
      },
      closeWindow: () => {
        if (events.at(-1) !== "close") events.push("close");
      },
      persistCredential: async (credential) => {
        persisted = credential;
        events.push("persist");
        return { provider: "leetcode", hasCredential: true };
      },
    });

    expect(events).toEqual([
      "clear",
      "create",
      "load",
      "fresh-cookies",
      "close",
      "persist",
    ]);
    expect(persisted).toBe('{"session":"session","csrfToken":"csrf"}');
    expect(JSON.stringify(result)).not.toContain("session");
  });

  test("keeps only authentication cookies within the credential bound", () => {
    expect(
      encodeCookieCredential([
        { name: "_ga", value: "analytics", httpOnly: false },
        { name: "sb-project-auth-token", value: "token", httpOnly: false },
        { name: "__session", value: "session", httpOnly: true },
      ]),
    ).toBe(
      '{"cookies":[{"name":"sb-project-auth-token","value":"token"},{"name":"__session","value":"session"}]}',
    );
    expect(encodeCookieCredential([])).toBeNull();
    expect(encodeCookieCredential([{ name: "bad", value: "x; y", httpOnly: true }])).toBeNull();
  });

  test("rejects missing or oversized LeetCode cookies", () => {
    expect(encodeLeetCodeCredential("", "csrf")).toBeNull();
    expect(encodeLeetCodeCredential("session", "")).toBeNull();
    expect(encodeLeetCodeCredential("x".repeat(1025), "csrf")).toBeNull();
  });
});
