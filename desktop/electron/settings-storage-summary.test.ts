import { expect, mock, test } from "bun:test";
import { tmpdir } from "node:os";
import path from "node:path";

mock.module("electron", () => ({
  app: {
    getPath: () => path.join(tmpdir(), "kosmos-settings-storage-summary-test"),
  },
}));

const { isPathInside } = await import("./settings-storage-summary");

test("isPathInside accepts same path and descendants only", () => {
  const root = path.resolve("C:", "Kosmos", "data");
  expect(isPathInside(root, root)).toBe(true);
  expect(isPathInside(root, path.join(root, "logs", "today.jsonl"))).toBe(true);
  expect(isPathInside(root, path.resolve("C:", "Kosmos", "other"))).toBe(false);
});
