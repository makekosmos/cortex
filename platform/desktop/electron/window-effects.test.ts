import { expect, test } from "bun:test";
import {
  normalizeLegacyBgMaterial,
  normalizeWindowEffects,
  resolveWindowMaterial,
} from "./window-effects";

test("KOSMOS_WINDOW_EFFECTS=flat resolves to no background material", () => {
  expect(resolveWindowMaterial("mica", { KOSMOS_WINDOW_EFFECTS: "flat" })).toBe("none");
});

test("KOSMOS_WINDOW_EFFECTS accepts mica and acrylic", () => {
  expect(resolveWindowMaterial("none", { KOSMOS_WINDOW_EFFECTS: "mica" })).toBe("mica");
  expect(resolveWindowMaterial("none", { KOSMOS_WINDOW_EFFECTS: "acrylic" })).toBe("acrylic");
});

test("KOSMOS_WINDOW_EFFECTS takes precedence over legacy KEPLER_BG_MATERIAL", () => {
  expect(
    resolveWindowMaterial("mica", {
      KOSMOS_WINDOW_EFFECTS: "flat",
      KEPLER_BG_MATERIAL: "acrylic",
    }),
  ).toBe("none");
});

test("legacy KEPLER_BG_MATERIAL remains supported when global flag is absent", () => {
  expect(resolveWindowMaterial("mica", { KEPLER_BG_MATERIAL: "none" })).toBe("none");
  expect(resolveWindowMaterial("none", { KEPLER_BG_MATERIAL: "acrylic" })).toBe("acrylic");
});

test("unknown env values fall back to caller default", () => {
  expect(resolveWindowMaterial("mica", { KOSMOS_WINDOW_EFFECTS: "off" })).toBe("mica");
  expect(normalizeWindowEffects("none")).toBeNull();
  expect(normalizeLegacyBgMaterial("flat")).toBeNull();
});
