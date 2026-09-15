/* oxlint-disable anti-slop/no-runtime-typeof -- this adapter translates dynamic test APIs. */
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  after,
  afterEach,
  before,
  beforeEach,
  describe,
  it,
  mock as nodeMock,
  test,
} from "node:test";

const partial = (actual, expected) => {
  if (!expected || typeof expected !== "object") {
    assert.deepStrictEqual(actual, expected);
    return;
  }
  if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual));
    expected.forEach((value, index) => partial(actual[index], value));
    return;
  }
  assert.ok(actual && typeof actual === "object");
  for (const [key, value] of Object.entries(expected)) partial(actual[key], value);
};

const matchers = (actual, negate = false) => {
  const check = (condition, message) => {
    if (negate) assert.equal(condition, false, message);
    else assert.ok(condition, message);
  };
  const api = {
    toBe(expected) {
      if (negate) assert.notStrictEqual(actual, expected);
      else assert.strictEqual(actual, expected);
    },
    toEqual(expected) {
      if (negate) assert.notDeepStrictEqual(actual, expected);
      else assert.deepStrictEqual(actual, expected);
    },
    toMatchObject(expected) {
      if (negate) assert.throws(() => partial(actual, expected));
      else partial(actual, expected);
    },
    toBeNull() {
      check(actual === null);
    },
    toBeUndefined() {
      check(actual === undefined);
    },
    toBeDefined() {
      check(actual !== undefined);
    },
    toBeTruthy() {
      check(Boolean(actual));
    },
    toBeFalsy() {
      check(!actual);
    },
    toBeInstanceOf(expected) {
      check(actual instanceof expected);
    },
    toBeGreaterThan(expected) {
      check(actual > expected);
    },
    toBeLessThan(expected) {
      check(actual < expected);
    },
    toBeLessThanOrEqual(expected) {
      check(actual <= expected);
    },
    toBeCloseTo(expected, precision = 2) {
      check(Math.abs(actual - expected) < 10 ** -precision / 2);
    },
    toContain(expected) {
      const contains =
        typeof actual?.includes === "function"
          ? actual.includes(expected) ||
            (typeof actual === "string" &&
              actual.replaceAll("\r\n", "\n").includes(String(expected).replaceAll("\r\n", "\n")))
          : actual?.has?.(expected);
      check(contains);
    },
    toStartWith(expected) {
      check(actual?.startsWith(expected));
    },
    toEndWith(expected) {
      check(actual?.endsWith(expected));
    },
    toHaveLength(expected) {
      check(actual?.length === expected);
    },
    toHaveProperty(property, expected) {
      const value = String(property)
        .split(".")
        .reduce((item, key) => item?.[key], actual);
      check(value !== undefined && (arguments.length < 2 || Object.is(value, expected)));
    },
    toMatch(expected) {
      check(
        typeof actual === "string" && expected.test
          ? expected.test(actual)
          : String(actual).includes(expected),
      );
    },
    toSatisfy(predicate) {
      check(predicate(actual));
    },
    toHaveBeenCalled() {
      check(actual?.mock?.calls?.length > 0);
    },
    toHaveBeenCalledTimes(expected) {
      check(actual?.mock?.calls?.length === expected);
    },
    toThrow(expected) {
      let thrown;
      try {
        actual();
      } catch (error) {
        thrown = error;
      }
      check(
        thrown !== undefined &&
          (expected === undefined ||
            (typeof expected === "function" && thrown instanceof expected) ||
            String(thrown).includes(expected) ||
            expected.test?.(String(thrown))),
      );
    },
  };
  const promiseMatcher = (resolve) =>
    new Proxy(
      {},
      {
        get(_target, property) {
          return (...args) => resolve().then((value) => matchers(value, negate)[property](...args));
        },
      },
    );
  Object.defineProperties(api, {
    not: { get: () => matchers(actual, !negate) },
    resolves: { get: () => promiseMatcher(() => Promise.resolve(actual)) },
    rejects: {
      get: () =>
        new Proxy(
          {},
          {
            get(_target, property) {
              return (...args) =>
                Promise.resolve(actual).then(
                  () => {
                    throw new assert.AssertionError({ message: "Expected promise to reject" });
                  },
                  (error) =>
                    matchers(
                      property === "toThrow"
                        ? () => {
                            throw error;
                          }
                        : error,
                      negate,
                    )[property](...args),
                );
            },
          },
        ),
    },
  });
  return api;
};

export const expect = (actual) => matchers(actual);
function callerFile() {
  const frame = new Error().stack?.split("\n").find((line) => line.includes(".test."));
  const match = frame?.match(/(?:file:\/\/\/.*?|[A-Z]:\\.*?)[.]test[.](?:ts|mjs)/);
  if (!match) return fileURLToPath(import.meta.url);
  return match[0].startsWith("file:") ? fileURLToPath(match[0]) : match[0];
}

const makeMock = (...args) => {
  const fn = nodeMock.fn(...args);
  fn.mockClear = () => {
    fn.mock.resetCalls();
    return fn;
  };
  return fn;
};
export const mock = Object.assign(makeMock, {
  fn: makeMock,
  module(specifier, factory) {
    const namedExports = typeof factory === "function" ? factory() : factory;
    const caller = callerFile();
    const resolved = specifier.startsWith(".")
      ? new URL(specifier, pathToFileURL(caller)).href
      : createRequire(caller).resolve(specifier);
    return nodeMock.module(resolved, { exports: namedExports });
  },
});
export const afterAll = after;
export const beforeAll = before;
export { after, afterEach, before, beforeEach, describe, it, test };
