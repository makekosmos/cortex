import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";

const source = readFileSync(path.join(import.meta.dirname, "build-package-components.mjs"), "utf8");

test("packaged GPUI components inherit the Desktop release version", () => {
  const icons = readFileSync(path.join(import.meta.dirname, "build-app-icons.mjs"), "utf8");
  expect(icons).toContain("png-to-ico");
});

test("components/manager is the manager-gpui exe staged under the packaged name", () => {
  expect(source).toContain('"cargo"');
  expect(source).toContain('"--locked"');
  expect(source).toContain("x86_64-pc-windows-msvc");
  expect(source).toContain("manager-gpui");
  expect(source).toContain('"Mundus Manager.exe"');
});

// KOS-265: Agenda/Memoria/Dictation install from the signed package catalog
// as native apps — they must never again be bundled into the installer stage.
test("the installer builds no bundled components besides Manager", () => {
  for (const component of ["agenda", "memoria", "dictation"]) {
    expect(source).not.toContain(`components\\${component}`);
    expect(source).not.toContain(`"components", "${component}"`);
  }
  for (const pin of ["agenda_gpui", "memoria_gpui", "dictation_gpui"]) {
    expect(source).not.toContain(pin);
  }
  for (const env of [
    "MUNDUS_AGENDA_GPUI_SRC",
    "MUNDUS_MEMORIA_GPUI_SRC",
    "MUNDUS_DICTATION_GPUI_SRC",
    "MUNDUS_AGENDA_VERSION",
    "MUNDUS_MEMORIA_VERSION",
    "MUNDUS_DICTATION_VERSION",
  ]) {
    expect(source).not.toContain(env);
  }
  for (const exe of ["Agenda.exe", "Memoria.exe", "Dictation.exe"]) {
    expect(source).not.toContain(exe);
  }
});
