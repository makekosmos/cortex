import { describe, expect, test } from "../../test-support/node-test.mjs";
import { updateSequentially } from "./updates-helpers";

describe("updates helpers", () => {
  test("updates available items sequentially and continues after an error", async () => {
    const order: number[] = [];
    const failures = await updateSequentially([1, 2, 3], async (item) => {
      order.push(item);
      return item === 2 ? "two failed" : null;
    });
    expect(order).toEqual([1, 2, 3]);
    expect(failures).toEqual(["two failed"]);
  });
});
