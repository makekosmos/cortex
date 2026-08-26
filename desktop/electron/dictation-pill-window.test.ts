import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import path from "node:path";

test("dictation pill restores its topmost z-order before every show", async () => {
  const source = await readFile(path.join(import.meta.dir, "dictation-pill.ts"), "utf8");
  const showPill = source.match(
    /function showPill\(\): void \{[\s\S]*?(?=\nfunction hidePill)/,
  )?.[0];

  expect(showPill).toBeDefined();

  const headlessGuard = showPill!.indexOf("if (isHeadless()) return;");
  const restoreTopmost = showPill!.indexOf('win.setAlwaysOnTop(true, "screen-saver", 1);');
  const showInactive = showPill!.indexOf("win.showInactive();");
  const moveTop = showPill!.indexOf("win.moveTop();");

  expect(headlessGuard).toBeGreaterThan(-1);
  expect(restoreTopmost).toBeGreaterThan(headlessGuard);
  expect(showInactive).toBeGreaterThan(restoreTopmost);
  expect(moveTop).toBeGreaterThan(showInactive);
});

test("dictation hotkey toggles directly without re-entering the command bus", async () => {
  const source = await readFile(path.join(import.meta.dir, "dictation-pill.ts"), "utf8");

  expect(source.match(/void toggleDictation\(\)/g)).toHaveLength(2);
  expect(source).not.toContain("dictationCommandInvoker");
});
