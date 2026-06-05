import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";

describe("focus command instant render", () => {
  test("does not gate the shell focus form behind async hydration", () => {
    // Regression: 2026-06-05. Built-in Shell command pages must paint immediately.
    const source = readFileSync(
      path.join(process.cwd(), "shell", "src", "components", "FocusCommandPanel.vue"),
      "utf8",
    );

    expect(source).not.toContain('v-if="loading"');
    expect(source).not.toContain("Загружаю фокус");
  });
});
