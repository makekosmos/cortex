import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

test("Manager opens even while Engine is recovering", () => {
  const source = readFileSync(new URL("./main.ts", import.meta.url), "utf8");
  expect(source).toContain("void waitForEngineReady();");
  expect(source).toContain("await createWindow();");
  expect(source).toContain('path.basename(process.execPath).toLowerCase() === "electron.exe"');
  expect(source.indexOf("void waitForEngineReady();")).toBeLessThan(source.indexOf("await createWindow();"));
  expect(source).toContain("process.env.VITE_DEV_SERVER_URL");
  expect(source).toContain("style-src 'self' 'unsafe-inline'");
  expect(source).toContain("style-src 'self'");
});
