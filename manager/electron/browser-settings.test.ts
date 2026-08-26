import { afterEach, describe, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  browserPartition,
  readBrowserDataPersistence,
  writeBrowserDataPersistence,
} from "./browser-settings";

const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-browser-settings-"));
const file = path.join(root, "browser.json");

afterEach(() => fs.rmSync(file, { force: true }));

describe("browser settings", () => {
  test("persists by default and uses an in-memory partition when disabled", () => {
    expect(readBrowserDataPersistence(file)).toBe(true);
    writeBrowserDataPersistence(file, false);
    expect(readBrowserDataPersistence(file)).toBe(false);
    expect(browserPartition("kosmos-manager-browser", true)).toBe(
      "persist:kosmos-manager-browser",
    );
    expect(browserPartition("kosmos-manager-browser", false)).toBe(
      "kosmos-manager-browser-private",
    );
  });
});
