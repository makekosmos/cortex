import { cn } from "@/lib/utils";

describe("cn", () => {
  it("merges tailwind classes", () => {
    expect(cn("p-2", "p-4", "text-sm")).toBe("p-4 text-sm");
  });

  it("skips falsy class values", () => {
    expect(cn("text-sm", false && "hidden", undefined)).toBe("text-sm");
  });
});
