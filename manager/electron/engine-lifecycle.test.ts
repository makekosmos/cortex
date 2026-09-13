import { describe, expect, test } from "bun:test";
import { waitForEngineStopped } from "./engine-lifecycle";

describe("Engine lifecycle boundary", () => {
  test("fails closed on access errors other than missing lock", async () => {
    await expect(
      waitForEngineStopped("engine.lock.json", async () => {
        // SAFETY: the test supplies the Node errno shape used by the lifecycle seam.
        const error = new Error("permission denied") as NodeJS.ErrnoException;
        error.code = "EACCES";
        throw error;
      }),
    ).rejects.toThrow(/permission denied/i);
  });
});
