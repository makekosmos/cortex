import { expect, mock, test } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { validateExtensionManifest } from "./extension-manifest-validation";

mock.module("electron", () => ({
  app: { isPackaged: false, getPath: () => os.tmpdir() },
}));
const { previewSource } = await import("./extension-installer");

const validManifest = {
  id: "com.kosmos.agenda",
  name: "Agenda",
  icon: "assets/icon.svg",
  entryHtml: "dist/index.html",
  native: { executable: "bin/agenda.exe" },
};

test("accepts optional manifest fields with safe relative paths", () => {
  expect(validateExtensionManifest(validManifest)).toMatchObject(validManifest);
  expect(validateExtensionManifest({ ...validManifest, icon: "assets\\icon.svg" }).icon).toBe(
    "assets/icon.svg",
  );
  expect(
    validateExtensionManifest({
      ...validManifest,
      native: { ...validManifest.native, args: ["--safe"] },
    }).native,
  ).toMatchObject({ args: ["--safe"] });
});

test("rejects malformed fields before path derivation", () => {
  expect(() => validateExtensionManifest(null)).toThrow("объект");
  expect(() => validateExtensionManifest({ id: 42, name: "Agenda" })).toThrow("manifest.id");
  expect(() => validateExtensionManifest({ id: "../escape", name: "Agenda" })).toThrow(
    "manifest.id",
  );
  expect(() => validateExtensionManifest({ id: "agenda", name: " " })).toThrow("manifest.name");

  for (const field of ["icon", "entryHtml", "preload"]) {
    expect(() =>
      validateExtensionManifest({ id: "agenda", name: "Agenda", [field]: "../x" }),
    ).toThrow(`manifest.${field}`);
    expect(() =>
      validateExtensionManifest({ id: "agenda", name: "Agenda", [field]: "C:\\x" }),
    ).toThrow(`manifest.${field}`);
    expect(() => validateExtensionManifest({ id: "agenda", name: "Agenda", [field]: 7 })).toThrow(
      `manifest.${field}`,
    );
  }

  expect(() =>
    validateExtensionManifest({ ...validManifest, native: { executable: "..\\x.exe" } }),
  ).toThrow("manifest.executable");
  expect(() => validateExtensionManifest({ ...validManifest, native: "bin/app.exe" })).toThrow(
    "manifest.native",
  );
  expect(() =>
    validateExtensionManifest({ ...validManifest, native: { executable: "app.exe", args: 42 } }),
  ).toThrow("manifest.native.args");
  expect(() =>
    validateExtensionManifest({
      ...validManifest,
      native: { executable: "app.exe", future: true },
    }),
  ).toThrow("manifest.native.future");
  expect(() =>
    validateExtensionManifest({ ...validManifest, keepAliveInBackground: "yes" }),
  ).toThrow("keepAliveInBackground");
});

test("previewSource validates and previews a real directory package", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-extension-"));
  try {
    await mkdir(path.join(root, "assets"), { recursive: true });
    await writeFile(path.join(root, "assets", "icon.svg"), "<svg />");
    await writeFile(
      path.join(root, "manifest.json"),
      JSON.stringify({ ...validManifest, keplerApiVersion: "^1.0.0" }),
    );

    const preview = await previewSource(root);
    expect(preview.manifest.id).toBe(validManifest.id);
    expect(preview.apiCompatError).toBeNull();
    expect(preview.iconDataUri).toStartWith("data:image/svg+xml;base64,");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("previewSource rejects a directory icon traversal", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-extension-"));
  try {
    await writeFile(
      path.join(root, "manifest.json"),
      JSON.stringify({ id: "agenda", name: "Agenda", icon: "../outside.svg" }),
    );
    await expect(previewSource(root)).rejects.toThrow("manifest.icon");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
