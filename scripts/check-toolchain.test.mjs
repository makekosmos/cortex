import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { expectedChannel, renderRustToolchain } from "./check-toolchain.mjs";

test("the checked-in rust-toolchain.toml is an exact render of toolchain.json", () => {
  const expected = expectedChannel(readFileSync("toolchain.json", "utf8"));
  const actual = readFileSync("rust-toolchain.toml", "utf8").replace(/\r\n/g, "\n");
  assert.equal(actual, renderRustToolchain(expected));
});

test("a drifted components line no longer matches the render", () => {
  const rendered = renderRustToolchain("1.95.0");
  const drifted = rendered.replace(
    'components = ["rustfmt", "clippy"]',
    'components = ["rustfmt"]',
  );
  assert.notEqual(drifted, rendered);
});

test("expectedChannel reads toolchain.json rust and rejects a missing pin", () => {
  assert.equal(expectedChannel('{"node": "24.15.0", "rust": "1.95.0"}'), "1.95.0");
  assert.throws(() => expectedChannel('{"node": "24.15.0"}'), /rust/);
});
