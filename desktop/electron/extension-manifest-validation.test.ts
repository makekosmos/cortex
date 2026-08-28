import { expect, test } from "bun:test";
import { validateExtensionManifest } from "./extension-manifest-validation";

test("accepts the minimal first-party extension manifest", () => {
  expect(
    validateExtensionManifest({
      id: "com.kosmos.agenda",
      name: "Agenda",
    }),
  ).toMatchObject({ id: "com.kosmos.agenda", name: "Agenda" });
});

test("rejects malformed package values before path derivation", () => {
  expect(() => validateExtensionManifest(null)).toThrow("объект");
  expect(() => validateExtensionManifest({ id: 42, name: "Agenda" })).toThrow("manifest.id");
  expect(() => validateExtensionManifest({ id: "../escape", name: "Agenda" })).toThrow(
    "manifest.id",
  );
  expect(() => validateExtensionManifest({ id: "com.kosmos.agenda", name: " " })).toThrow(
    "manifest.name",
  );
});
