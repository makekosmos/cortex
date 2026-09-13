import { expect, test } from "../test-support/node-test.mjs";
import {
  normalizeLegacyBgMaterial,
  normalizeWindowEffects,
  resolveWindowMaterial,
} from "./window-effects";

test("KOSMOS_WINDOW_EFFECTS=flat resolves to no background material", () => {
  expect(resolveWindowMaterial("mica", { KOSMOS_WINDOW_EFFECTS: "flat" })).toBe("none");
});

test("KOSMOS_WINDOW_EFFECTS mica/acrylic are ignored while flat mode is forced", () => {
  expect(resolveWindowMaterial("none", { KOSMOS_WINDOW_EFFECTS: "mica" })).toBe("none");
  expect(resolveWindowMaterial("none", { KOSMOS_WINDOW_EFFECTS: "acrylic" })).toBe("none");
});

test("KOSMOS_WINDOW_EFFECTS takes precedence over legacy KEPLER_BG_MATERIAL", () => {
  expect(
    resolveWindowMaterial("mica", {
      KOSMOS_WINDOW_EFFECTS: "flat",
      KEPLER_BG_MATERIAL: "acrylic",
    }),
  ).toBe("none");
});

test("legacy KEPLER_BG_MATERIAL is ignored while flat mode is forced", () => {
  expect(resolveWindowMaterial("mica", { KEPLER_BG_MATERIAL: "none" })).toBe("none");
  expect(resolveWindowMaterial("none", { KEPLER_BG_MATERIAL: "acrylic" })).toBe("none");
});

test("unknown env values still resolve to forced flat mode", () => {
  expect(resolveWindowMaterial("mica", { KOSMOS_WINDOW_EFFECTS: "off" })).toBe("none");
  expect(normalizeWindowEffects("none")).toBeNull();
  expect(normalizeLegacyBgMaterial("flat")).toBeNull();
});
