import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import path from "node:path";

// Regression: 2026-07-10. The fullscreen blocked-app overlay must leave the
// desktop transparent outside its intentional renderer edge gradient/popup.
test("focus blocked-app overlay uses a transparent native surface", async () => {
  const windowSource = await readFile(path.join(import.meta.dir, "focus-overlay.ts"), "utf8");
  const viewSource = await readFile(
    path.join(import.meta.dir, "..", "src", "views", "FocusBlockOverlay.vue"),
    "utf8",
  );

  expect(windowSource).toContain("transparent: true");
  expect(windowSource).toContain('backgroundColor: "#00000000"');
  expect(windowSource).not.toContain("backgroundMaterialOption(");
  expect(windowSource).not.toContain("applyWindowMaterial(");
  expect(windowSource).not.toMatch(/\bbackgroundMaterial\s*:/);
  expect(windowSource).not.toContain("setBackgroundMaterial(");

  expect(viewSource).toContain('class="focus-overlay__edges"');
  expect(viewSource).toContain("edgesOn.value = true");
  expect(viewSource).toContain("linear-gradient(");
  expect(viewSource).toContain('class="focus-overlay__popup"');
  expect(viewSource).toContain("--feedback-accent: var(--status-warning)");
  expect(viewSource).toContain("--feedback-accent: var(--status-success)");
  expect(viewSource).toContain("Задача выполнена");
  expect(windowSource).toContain('kind: "blocked"');
  expect(windowSource).toContain('kind: "completed"');
});
