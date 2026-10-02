import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { expectedChannel, pinnedChannel, renderRustToolchain } from "./check-toolchain.mjs";

test("the checked-in rust-toolchain.toml matches toolchain.json", () => {
  const expected = expectedChannel(readFileSync("toolchain.json", "utf8"));
  const actual = pinnedChannel(readFileSync("rust-toolchain.toml", "utf8"));
  assert.equal(actual, expected);
});

test("pinnedChannel reads the channel and rejects a missing pin", () => {
  assert.equal(pinnedChannel('[toolchain]\nchannel = "1.95.0"\n'), "1.95.0");
  assert.throws(() => pinnedChannel('[toolchain]\nprofile = "minimal"\n'), /channel/);
});

test("expectedChannel reads toolchain.json rust and rejects a missing pin", () => {
  assert.equal(expectedChannel('{"node": "24.15.0", "rust": "1.95.0"}'), "1.95.0");
  assert.throws(() => expectedChannel('{"node": "24.15.0"}'), /rust/);
});

test("renderRustToolchain round-trips the channel", () => {
  assert.equal(pinnedChannel(renderRustToolchain("1.95.0")), "1.95.0");
});
