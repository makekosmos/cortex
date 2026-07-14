import { expect, test } from "bun:test";
import { dominantImageColor } from "./image-dominant-color";

test("dominantImageColor prefers the largest chromatic bucket and falls back for grayscale", () => {
  const pixels = new Uint8Array([
    132, 82, 38, 255, 141, 91, 45, 255, 136, 86, 40, 255, 138, 88, 42, 255, 130, 80, 36, 255, 140,
    90, 44, 255, 110, 110, 110, 255, 111, 111, 111, 255, 112, 112, 112, 255, 113, 113, 113, 255,
    114, 114, 114, 255,
  ]);
  expect(dominantImageColor(pixels, 4)).toBe("rgb(136 86 41)");
  expect(dominantImageColor(new Uint8Array([90, 90, 90, 255]), 4)).toBe("rgb(90 90 90)");
  expect(dominantImageColor(new Uint8Array([0, 0, 0, 0]), 4)).toBeNull();
});
