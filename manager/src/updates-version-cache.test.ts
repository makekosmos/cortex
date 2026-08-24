import { describe, expect, test } from "bun:test";
import { createDesktopVersionCache } from "./updates-version-cache";

describe("createDesktopVersionCache", () => {
  test("loads the immutable desktop version once per app session", async () => {
    let calls = 0;
    const version = createDesktopVersionCache(async () => {
      calls += 1;
      return "0.8.17";
    });

    expect(await version()).toBe("0.8.17");
    expect(await version()).toBe("0.8.17");
    expect(calls).toBe(1);
  });
});
