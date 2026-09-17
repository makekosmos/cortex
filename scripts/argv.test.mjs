import assert from "node:assert/strict";
import test from "node:test";
import { flag } from "./argv.mjs";

test("flag returns the token after --name", () => {
  assert.equal(flag(["--apps-root", "/workspace"], "--apps-root"), "/workspace");
  assert.equal(flag(["--report", "a.md", "--report", "b.md"], "--report"), "a.md");
});

test("flag returns null when the flag is absent, never argv[0]", () => {
  const argv = ["node", "linux-smoke.mjs", "--apps-root", "/workspace"];
  assert.equal(flag(argv, "--report"), null);
  assert.equal(flag([...argv, "--report"], "--report"), undefined);
});
