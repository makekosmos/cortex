import { expect, test } from "bun:test";
import { imageDimensions } from "./image-dimensions";

test("imageDimensions reads PNG and standard JPEG SOF dimensions", () => {
  const png = new Uint8Array([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52, 0, 0, 2, 0,
    0, 0, 3, 0,
  ]);
  expect(imageDimensions(png)).toEqual({ width: 512, height: 768 });

  for (const marker of [
    0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
  ]) {
    const jpeg = Buffer.from([
      0xff,
      0xd8,
      0xff,
      0xe0,
      0,
      4,
      0,
      0,
      0xff,
      marker,
      0,
      7,
      8,
      0x01,
      0x2c,
      0x02,
      0x58,
    ]);
    expect(imageDimensions(jpeg)).toEqual({ width: 600, height: 300 });
  }

  expect(imageDimensions(new Uint8Array([0x89, 0x50, 0x4e]))).toBeNull();
  expect(imageDimensions(new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 0, 20]))).toBeNull();
  expect(imageDimensions(new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]))).toBeNull();
});
