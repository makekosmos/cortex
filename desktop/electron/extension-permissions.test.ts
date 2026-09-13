import { describe, expect, test } from "../test-support/node-test.mjs";
import { assertExtensionHostPermission } from "./extension-permissions";

const check = (capability: "userData.read" | "userData.write", permissions: string[]) =>
  assertExtensionHostPermission({
    extensionId: "com.kosmos.memoria",
    source: "user",
    manifestPermissions: permissions,
    capability,
  });

describe("extension userData permissions", () => {
  test("maps filesystem read/write manifest grants to isolated userData operations", () => {
    expect(() => check("userData.read", ["filesystem.read"])).not.toThrow();
    expect(() => check("userData.write", ["filesystem.write"])).not.toThrow();
  });

  test("does not allow a read grant to write or a write grant to read", () => {
    expect(() => check("userData.write", ["filesystem.read"])).toThrow();
    expect(() => check("userData.read", ["filesystem.write"])).toThrow();
  });
});
