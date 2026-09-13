import { describe, expect, test } from "bun:test";
import { parseOpenAppRequest, sendNavigationWhenReady } from "./app-navigation";

describe("renderer app navigation boundary", () => {
  test("accepts a safe app id and encoded note route", () => {
    expect(parseOpenAppRequest({ id: "com.kosmos.memoria", route: "/note/note%2F42" })).toEqual({
      ok: true,
      request: { id: "com.kosmos.memoria", route: "/note/note%2F42" },
    });
  });

  test("rejects malformed ids and routes before opening an app", () => {
    expect(parseOpenAppRequest({ id: "../../memoria", route: "/note/42" })).toMatchObject({
      ok: false,
    });
    expect(parseOpenAppRequest({ id: "com.kosmos.memoria", route: "https://evil.test" })).toEqual({
      ok: false,
      message: "Некорректный маршрут приложения.",
    });
    expect(parseOpenAppRequest({ id: "com.kosmos.memoria", route: "/note/%" })).toEqual({
      ok: false,
      message: "Некорректный маршрут приложения.",
    });
    expect(parseOpenAppRequest({ id: "com.kosmos.memoria", route: "//evil.test/note/42" })).toEqual(
      {
        ok: false,
        message: "Некорректный маршрут приложения.",
      },
    );
  });

  test("allows opening an app without a route", () => {
    expect(parseOpenAppRequest({ id: "com.kosmos.memoria" })).toEqual({
      ok: true,
      request: { id: "com.kosmos.memoria" },
    });
  });

  test("waits for a cold window before delivering a concurrent route", async () => {
    let release!: (ready: boolean) => void;
    const readiness = new Promise<boolean>((resolve) => {
      release = resolve;
    });
    let sent = false;
    const delivery = sendNavigationWhenReady(readiness, () => {
      sent = true;
    });

    await Promise.resolve();
    expect(sent).toBe(false);
    release(true);
    await expect(delivery).resolves.toBe(true);
    expect(sent).toBe(true);
  });

  test("does not deliver a route after a failed cold window", async () => {
    let sent = false;
    await expect(
      sendNavigationWhenReady(Promise.resolve(false), () => {
        sent = true;
      }),
    ).resolves.toBe(false);
    expect(sent).toBe(false);
  });
});
