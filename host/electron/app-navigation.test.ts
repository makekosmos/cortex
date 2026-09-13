import { describe, expect, test } from "bun:test";
import { parseOpenAppRequest } from "./app-navigation";

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
  });

  test("allows opening an app without a route", () => {
    expect(parseOpenAppRequest({ id: "com.kosmos.memoria" })).toEqual({
      ok: true,
      request: { id: "com.kosmos.memoria" },
    });
  });
});
