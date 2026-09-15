import { expect, test } from "../test-support/node-test.mjs";
import { bufferToArrayBuffer, parseAppIconRequestUrl } from "./app-icon-protocol";

test("parseAppIconRequestUrl accepts encoded app ids", () => {
  // Regression: 2026-06-10. kosmos-icon renderer URLs must resolve back to app ids.
  expect(
    parseAppIconRequestUrl("kosmos-icon://app/Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"),
  ).toBe("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App");
  expect(parseAppIconRequestUrl("kosmos-icon://app/a%2Fb%25c")).toBe("a/b%c");
});

test("parseAppIconRequestUrl rejects non-app icon URLs", () => {
  expect(parseAppIconRequestUrl("kosmos-icon://file/app-id")).toBeNull();
  expect(parseAppIconRequestUrl("https://app/app-id")).toBeNull();
  expect(parseAppIconRequestUrl("kosmos-icon://app/")).toBeNull();
});

test("bufferToArrayBuffer preserves sliced buffer bytes", () => {
  const source = Buffer.from([0, 1, 2, 3, 4]);
  const sliced = source.subarray(1, 4);
  expect([...new Uint8Array(bufferToArrayBuffer(sliced))]).toEqual([1, 2, 3]);
});
