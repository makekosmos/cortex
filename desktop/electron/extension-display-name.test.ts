import { expect, test } from "../test-support/node-test.mjs";
import { extensionDisplayName } from "./extension-display-name";

test("uses current names for legacy extension identifiers", () => {
  expect(extensionDisplayName("eden", "Eden")).toBe("Memoria");
  expect(extensionDisplayName("delphi", "Delphi")).toBe("Agenda");
  expect(extensionDisplayName("arrancador", "Arrancador")).toBe("Arcadia");
  expect(extensionDisplayName("shell", "Kosmos Shell")).toBe("Kosmos Shell");
});
