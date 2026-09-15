import { expect, test } from "../test-support/node-test.mjs";
import { existsSync } from "node:fs";

test("legacy extension resolver is absent", () => {
  expect(existsSync(new URL("./extension-roots.ts", import.meta.url))).toBe(false);
});
