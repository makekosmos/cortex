import { describe, expect, test } from "bun:test";
import { encodeLeetCodeCredential, runLeetCodeLogin } from "./leetcode-login-flow";

describe("Manager LeetCode login flow", () => {
  test("clears stale cookies, closes before Engine persistence, and never returns the credential", async () => {
    const events: string[] = [];
    let persisted = "";
    const result = await runLeetCodeLogin({
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

    expect(events).toEqual(["clear", "create", "load", "fresh-cookies", "close", "persist"]);
    expect(persisted).toBe('{"session":"session","csrfToken":"csrf"}');
    expect(result).toEqual({ provider: "leetcode", hasCredential: true });
    expect(JSON.stringify(result)).not.toContain("session");
  });

  test("rejects missing or oversized cookies before they can reach Engine", () => {
    expect(encodeLeetCodeCredential("", "csrf")).toBeNull();
    expect(encodeLeetCodeCredential("session", "")).toBeNull();
    expect(encodeLeetCodeCredential("x".repeat(1025), "csrf")).toBeNull();
    expect(encodeLeetCodeCredential("session", "x".repeat(1025))).toBeNull();
  });
});
