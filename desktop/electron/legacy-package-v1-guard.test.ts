import { expect, test } from "bun:test";
import { existsSync } from "node:fs";

test("legacy extension resolver is absent", () => {
  expect(existsSync(new URL("./extension-roots.ts", import.meta.url))).toBe(false);
});
