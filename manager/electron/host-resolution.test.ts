import { describe, expect, test } from "../test-support/node-test.mjs";
import path from "node:path";
import { resolvePackagedHostExecutable, resolvePackagedRuntimeExecutable } from "./host-resolution";

describe("Manager packaged Host resolution", () => {
  test("walks from nested Manager resources to its Desktop Host sibling", () => {
    const desktopResources = path.join("C:\\", "Kosmos", "resources");
    const managerResources = path.join(desktopResources, "components", "manager", "resources");
    expect(resolvePackagedHostExecutable(managerResources)).toBe(
      path.join(desktopResources, "components", "host", "Kosmos Package Host.exe"),
    );
  });

  test("walks from nested Manager resources to the Desktop Runtime", () => {
    const desktopResources = path.join("C:\\", "Kosmos", "resources");
    const managerResources = path.join(desktopResources, "components", "manager", "resources");
    expect(resolvePackagedRuntimeExecutable(managerResources)).toBe(
      path.join(desktopResources, "Kosmos Runtime.exe"),
    );
  });
});
