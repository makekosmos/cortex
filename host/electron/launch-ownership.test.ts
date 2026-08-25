import { describe, expect, test } from "bun:test";
import { LaunchOwnership } from "./launch-ownership";

type Claim = ReturnType<LaunchOwnership["claim"]>["current"];

const claim = (
  ownership: LaunchOwnership,
  id: string,
  owner: NonNullable<unknown>,
  launchId: string,
): Claim =>
  ownership.claim(id, owner, 1, launchId).current;

describe("LaunchOwnership", () => {
  test("normal release clears its local claim and revokes exactly once", async () => {
    const ownership = new LaunchOwnership();
    const current = claim(ownership, "com.kosmos.demo", {}, "launch-one");
    const revoked: string[] = [];

    expect(await ownership.release(current, async (launchId) => void revoked.push(launchId))).toBe(
      true,
    );
    expect(ownership.size()).toBe(0);
    expect(revoked).toEqual(["launch-one"]);
    expect(await ownership.release(current, async (launchId) => void revoked.push(launchId))).toBe(
      false,
    );
  });

  test("renderer or navigation terminal seam releases its current claim", async () => {
    const ownership = new LaunchOwnership();
    const current = claim(ownership, "com.kosmos.demo", {}, "terminal-launch");
    const revoked: string[] = [];

    await ownership.release(current, async (launchId) => void revoked.push(launchId));

    expect(ownership.size()).toBe(0);
    expect(revoked).toEqual(["terminal-launch"]);
  });

  test("stale generation cannot release or revoke its replacement", async () => {
    const ownership = new LaunchOwnership();
    const stale = claim(ownership, "com.kosmos.demo", {}, "old-launch");
    const replacement = ownership.claim("com.kosmos.demo", {}, 2, "new-launch");
    const revoked: string[] = [];

    expect(replacement.replaced?.launchId).toBe("old-launch");
    expect(await ownership.release(stale, async (launchId) => void revoked.push(launchId))).toBe(
      false,
    );
    expect(ownership.current("com.kosmos.demo")).toEqual(replacement.current);
    expect(revoked).toEqual([]);
  });

  test("a rejected revoke still clears local state and permits a later release", async () => {
    const ownership = new LaunchOwnership();
    const first = claim(ownership, "com.kosmos.demo", {}, "first-launch");
    await ownership.release(first, async () => Promise.reject(new Error("network down")));
    const second = claim(ownership, "com.kosmos.demo", {}, "second-launch");
    const revoked: string[] = [];

    expect(ownership.size()).toBe(1);
    expect(await ownership.release(second, async (launchId) => void revoked.push(launchId))).toBe(
      true,
    );
    expect(revoked).toEqual(["second-launch"]);
  });

  test("drain clears and revokes every current launch exactly once despite rejection", async () => {
    const ownership = new LaunchOwnership();
    claim(ownership, "one", {}, "launch-one");
    claim(ownership, "two", {}, "launch-two");
    const revoked: string[] = [];

    await ownership.drain(async (launchId) => {
      revoked.push(launchId);
      if (launchId === "launch-one") throw new Error("rejected");
    });

    expect(ownership.size()).toBe(0);
    expect(revoked.sort()).toEqual(["launch-one", "launch-two"]);
    await ownership.drain(async (launchId) => void revoked.push(launchId));
    expect(revoked.sort()).toEqual(["launch-one", "launch-two"]);
  });
});
