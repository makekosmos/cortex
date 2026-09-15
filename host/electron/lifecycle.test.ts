import { describe, expect, test } from "../test-support/node-test.mjs";
import { HostLifecycle } from "./lifecycle";

describe("HostLifecycle", () => {
  test("timeout 0 exits after final window", () => {
    let exits = 0;
    const lifecycle = new HostLifecycle(
      () => {
        exits += 1;
      },
      { setTimeout, clearTimeout },
      0,
    );
    lifecycle.opened();
    lifecycle.closed();
    expect(exits).toBe(1);
  });

  test("warm timeout is cancelled when a window reopens", () => {
    let exits = 0;
    let callback: (() => void) | undefined;
    const timer = {
      setTimeout: (fn: () => void) => {
        callback = fn;
        return 1;
      },
      clearTimeout: () => {
        callback = undefined;
      },
    };
    const lifecycle = new HostLifecycle(
      () => {
        exits += 1;
      },
      timer,
      300,
    );
    lifecycle.opened();
    lifecycle.closed();
    lifecycle.opened();
    callback?.();
    expect(exits).toBe(0);
  });

  test("warm timeout expires after exactly five minutes", () => {
    let exits = 0;
    let callback: (() => void) | undefined;
    let delay: number | undefined;
    const timer = {
      setTimeout: (fn: () => void, ms: number) => {
        callback = fn;
        delay = ms;
        return 1;
      },
      clearTimeout: () => undefined,
    };
    const lifecycle = new HostLifecycle(
      () => {
        exits += 1;
      },
      timer,
      300,
    );
    lifecycle.opened();
    lifecycle.closed();
    expect(delay).toBe(300_000);
    callback?.();
    expect(exits).toBe(1);
  });

  test("default warm timeout matches Engine default", () => {
    let delay: number | undefined;
    const timer = {
      setTimeout: (_fn: () => void, ms: number) => {
        delay = ms;
        return 1;
      },
      clearTimeout: () => undefined,
    };
    const lifecycle = new HostLifecycle(() => undefined, timer);
    lifecycle.opened();
    lifecycle.closed();
    expect(delay).toBe(300_000);
  });
});
