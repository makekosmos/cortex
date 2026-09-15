/* oxlint-disable anti-slop/no-runtime-typeof -- this adapter translates dynamic test APIs. */
import {
  mock as nodeMock,
  after,
  afterEach,
  before,
  beforeEach,
  describe,
  it,
  test as nodeTest,
} from "node:test";
import { existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { expect } from "../../test-support/node-test.mjs";

function callerFile() {
  const frame = new Error().stack?.split("\n").find((line) => line.includes(".test."));
  const match = frame?.match(/(?:file:\/\/\/.*?|[A-Z]:\\.*?)[.]test[.](?:ts|mjs)/);
  return match
    ? match[0].startsWith("file:")
      ? fileURLToPath(match[0])
      : match[0]
    : fileURLToPath(import.meta.url);
}
const makeMock = (...args) => {
  const fn = nodeMock.fn(...args);
  fn.mockClear = () => {
    fn.mock.resetCalls();
    return fn;
  };
  return fn;
};
let previousTest = Promise.resolve();
const test = (name, options, callback) => {
  if (typeof options === "function") {
    callback = options;
    options = {};
  }
  return nodeTest(name, { ...options, concurrency: false }, async (context) => {
    const waitForPrevious = previousTest;
    let release;
    previousTest = new Promise((resolve) => {
      release = resolve;
    });
    await waitForPrevious;
    try {
      return await callback(context);
    } finally {
      release();
    }
  });
};
export const mock = Object.assign(makeMock, {
  fn: makeMock,
  module(specifier, factory) {
    const caller = callerFile();
    const resolved = specifier.startsWith(".")
      ? ["", ".ts", ".mjs", ".js"]
          .map((suffix) => new URL(`${specifier}${suffix}`, pathToFileURL(caller)))
          .find((url) => existsSync(fileURLToPath(url))).href
      : specifier;
    return nodeMock.module(resolved, {
      exports: typeof factory === "function" ? factory() : factory,
    });
  },
});
export const afterAll = after;
export const beforeAll = before;
export { after, afterEach, before, beforeEach, describe, expect, it, test };
