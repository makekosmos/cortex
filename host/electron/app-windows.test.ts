import { describe, expect, test } from "../test-support/node-test.mjs";
import {
  auxWindowKey,
  auxWindowStateFileName,
  parseAuxWindowSpec,
  parseAuxWindowState,
} from "./app-windows";

describe("auxiliary app window spec", () => {
  test("accepts a minimal sticker-style request with defaults", () => {
    const parsed = parseAuxWindowSpec({ key: "sticker:note-1" });
    expect(parsed).toEqual({
      ok: true,
      spec: {
        key: "sticker:note-1",
        width: 380,
        height: 480,
        minWidth: 240,
        minHeight: 180,
      },
    });
  });

  test("accepts explicit geometry, route and pin flag", () => {
    const parsed = parseAuxWindowSpec({
      key: "sticker:note%3Aabc",
      route: "/sticker/note%3Aabc",
      width: 420.6,
      height: 320,
      minWidth: 200,
      minHeight: 160,
      alwaysOnTop: true,
    });
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.spec.width).toBe(421);
    expect(parsed.spec.minWidth).toBe(200);
    expect(parsed.spec.alwaysOnTop).toBe(true);
    expect(parsed.spec.route).toBe("/sticker/note%3Aabc");
  });

  test("rejects unsafe keys and routes", () => {
    for (const key of ["", " sticker", "a/b", "a\\b", "a?b", "a#b", "a".repeat(65), "💡"]) {
      const parsed = parseAuxWindowSpec({ key });
      expect(parsed.ok).toBe(false);
    }
    expect(parseAuxWindowSpec({ key: "k", route: "https://evil.example" }).ok).toBe(false);
    expect(parseAuxWindowSpec({ key: "k", route: "//evil.example" }).ok).toBe(false);
    expect(parseAuxWindowSpec({ key: "k", route: "sticker/x" }).ok).toBe(false);
    expect(parseAuxWindowSpec({ key: "k", route: "/%E0%A4%A" }).ok).toBe(false);
    expect(parseAuxWindowSpec(undefined).ok).toBe(false);
    expect(parseAuxWindowSpec({ key: "k", alwaysOnTop: "yes" }).ok).toBe(false);
  });

  test("clamps dimensions to screen-sane bounds and caps minimums", () => {
    const parsed = parseAuxWindowSpec({
      key: "k",
      width: 99999,
      height: -5,
      minWidth: 9999,
      minHeight: 10,
    });
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.spec.width).toBe(3840);
    expect(parsed.spec.height).toBe(120);
    expect(parsed.spec.minWidth).toBe(3840);
    expect(parsed.spec.minHeight).toBe(120);
  });

  test("state file names are stable, unique and path-safe", () => {
    const a = auxWindowStateFileName("sticker:note-1");
    const b = auxWindowStateFileName("sticker:note-2");
    expect(a).toMatch(/^[0-9a-f]{16}\.json$/);
    expect(a).not.toBe(b);
    expect(auxWindowStateFileName("sticker:note-1")).toBe(a);
  });

  test("window keys are composite per app", () => {
    expect(auxWindowKey("com.kosmos.memoria", "sticker:a")).toBe("com.kosmos.memoria sticker:a");
  });

  test("persisted state parsing drops malformed fields", () => {
    expect(parseAuxWindowState({ x: 10.4, y: -20, width: "wide", alwaysOnTop: true })).toEqual({
      x: 10,
      y: -20,
      alwaysOnTop: true,
    });
    expect(parseAuxWindowState(null)).toEqual({});
    expect(parseAuxWindowState("junk")).toEqual({});
  });
});
