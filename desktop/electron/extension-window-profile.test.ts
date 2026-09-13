import { expect, test } from "../test-support/node-test.mjs";
import {
  SETTINGS_WINDOW_SIZE,
  settingsWindowBounds,
  windowProfileTraits,
} from "./extension-window-profile";

test("settingsWindowBounds centers the fixed settings size on the work area", () => {
  const bounds = settingsWindowBounds({ width: 1920, height: 1080 });
  expect(bounds.width).toBe(SETTINGS_WINDOW_SIZE.width);
  expect(bounds.height).toBe(SETTINGS_WINDOW_SIZE.height);
  expect(bounds.minWidth).toBe(SETTINGS_WINDOW_SIZE.minWidth);
  expect(bounds.minHeight).toBe(SETTINGS_WINDOW_SIZE.minHeight);
  expect(bounds.x).toBe(Math.round((1920 - SETTINGS_WINDOW_SIZE.width) / 2));
  expect(bounds.y).toBe(Math.round((1080 - SETTINGS_WINDOW_SIZE.height) / 2));
});

test("settings profile does not persist geometry and is non-maximizable acrylic", () => {
  const traits = windowProfileTraits("settings");
  expect(traits.persistWindowState).toBe(false);
  expect(traits.maximizable).toBe(false);
  expect(traits.fullscreenable).toBe(false);
  expect(traits.forceAcrylic).toBe(true);
});

test("default profile keeps geometry persistence, maximize, and manifest backdrop", () => {
  const traits = windowProfileTraits("default");
  expect(traits.persistWindowState).toBe(true);
  expect(traits.maximizable).toBe(true);
  expect(traits.fullscreenable).toBe(true);
  expect(traits.forceAcrylic).toBe(false);
});
