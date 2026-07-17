import { describe, expect, test } from "bun:test";
import { runLeetCodeLogin } from "./leetcode-auth-flow";

describe("LeetCode login flow", () => {
  test("clears stale cookies and closes the browser before credential verification", async () => {
    // Regression: 2026-07-17. Stale cookies were accepted and the login window stayed open
    // throughout the network verification performed by integrations.set_credential.
    const events: string[] = [];
    const result = await runLeetCodeLogin({
      clearCookies: async () => {
        events.push("clear");
      },
      createWindow: () => {
        events.push("create");
        return {};
      },
      loadLogin: async () => {
        events.push("load");
      },
      waitForCredential: async () => {
        events.push("credential");
        return "fresh";
      },
      closeWindow: () => {
        if (events.at(-1) !== "close") events.push("close");
      },
      persistCredential: async () => {
        events.push("persist");
        return "snapshot";
      },
    });

    expect(result).toBe("snapshot");
    expect(events).toEqual(["clear", "create", "load", "credential", "close", "persist"]);
  });
});
