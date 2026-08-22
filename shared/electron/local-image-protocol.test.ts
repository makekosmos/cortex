import { expect, test } from "bun:test";
import { localImageUrl, parseLocalImageRequestUrl } from "./local-image-protocol";

test("parseLocalImageRequestUrl round-trips absolute local image paths", () => {
  const windowsPath = "C:\\vault\\images\\diagram.png";
  const unixPath = "/vault/images/diagram.png";

  expect(parseLocalImageRequestUrl(localImageUrl(windowsPath))).toBe(windowsPath);
  expect(parseLocalImageRequestUrl(localImageUrl(unixPath))).toBe(unixPath);
});

test("parseLocalImageRequestUrl rejects non-local or relative URLs", () => {
  expect(parseLocalImageRequestUrl("kosmos-local-image://file/")).toBeNull();
  expect(parseLocalImageRequestUrl("kosmos-local-image://other/%2Ftmp%2Fx.png")).toBeNull();
  expect(parseLocalImageRequestUrl("file:///tmp/x.png")).toBeNull();
  expect(parseLocalImageRequestUrl("../images/x.png")).toBeNull();
  expect(parseLocalImageRequestUrl(localImageUrl("C:\\vault\\notes\\draft.md"))).toBeNull();
});
