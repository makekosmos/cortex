import { describe, expect, test } from "bun:test";
import {
  encodeGreatFrontendCredential,
  encodeLeetCodeCredential,
  runIntegrationLogin,
} from "./integration-login-credential";

describe("Manager integration login flow", () => {
  test("closes only after Engine persistence and never returns the credential", async () => {
    const events: string[] = [];
    let persisted = "";
    const result = await runIntegrationLogin({
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
      "create",
      "load",
      "fresh-cookies",
      "persist",
      "close",
    ]);
    expect(persisted).toBe('{"session":"session","csrfToken":"csrf"}');
    expect(JSON.stringify(result)).not.toContain("session");
  });

  test("keeps only the GreatFrontEnd session token within the keyring bound", () => {
    expect(
      encodeGreatFrontendCredential([
        { name: "_ga", value: "analytics", httpOnly: false },
        { name: "csrf-token", value: "csrf", httpOnly: true },
        { name: "supabase-auth-token", value: "session", httpOnly: false },
      ]),
    ).toBe("session");
    expect(encodeGreatFrontendCredential([])).toBeNull();
    expect(
      encodeGreatFrontendCredential([
        { name: "supabase-auth-token", value: "x; y", httpOnly: true },
      ]),
    ).toBeNull();
  });

  test("rejects missing or oversized LeetCode cookies", () => {
    expect(encodeLeetCodeCredential("", "csrf")).toBeNull();
    expect(encodeLeetCodeCredential("session", "")).toBeNull();
    expect(encodeLeetCodeCredential("x".repeat(1025), "csrf")).toBeNull();
  });
});
