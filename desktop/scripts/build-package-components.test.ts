import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";

const source = readFileSync(path.join(import.meta.dirname, "build-package-components.mjs"), "utf8");

test("packaged GPUI components inherit the Desktop release version", () => {
  expect(source).toContain('release-versions.json"), "utf8"');
  const icons = readFileSync(path.join(import.meta.dirname, "build-app-icons.mjs"), "utf8");
  expect(icons).toContain("png-to-ico");
});

test("components/manager is the manager-gpui exe staged under the packaged name", () => {
  expect(source).toContain('"cargo"');
  expect(source).toContain('"--locked"');
  expect(source).toContain("x86_64-pc-windows-msvc");
  expect(source).toContain("manager-gpui");
  expect(source).toContain('"Kosmos Manager.exe"');
});

test("components/agenda is the pinned agenda-gpui exe staged under the packaged name", () => {
  const pins = JSON.parse(
    readFileSync(path.join(import.meta.dirname, "..", "component-pins.json"), "utf8"),
  );
  expect(pins.agenda_gpui.repository).toBe("makekosmos/agenda-gpui");
  expect(pins.agenda_gpui.commit).toMatch(/^[0-9a-f]{40}$/);
  expect(source).toContain("component-pins.json");
  expect(source).toContain("KOSMOS_AGENDA_GPUI_SRC");
  expect(source).toContain('"rev-parse", "HEAD"');
  expect(source).toContain("x86_64-pc-windows-msvc");
  expect(source).toContain("agenda-gpui");
  expect(source).toContain("KOSMOS_AGENDA_VERSION");
  expect(source).toContain('"Kosmos Agenda.exe"');
});

test("components/memoria is the pinned memoria-gpui exe staged under the packaged name", () => {
  const pins = JSON.parse(
    readFileSync(path.join(import.meta.dirname, "..", "component-pins.json"), "utf8"),
  );
  expect(pins.memoria_gpui.repository).toBe("makekosmos/memoria-gpui");
  expect(pins.memoria_gpui.commit).toMatch(/^[0-9a-f]{40}$/);
  expect(source).toContain("KOSMOS_MEMORIA_GPUI_SRC");
  expect(source).toContain("memoria-gpui");
  expect(source).toContain("KOSMOS_MEMORIA_VERSION");
  expect(source).toContain('"Kosmos Memoria.exe"');
});

test("components/dictation is the pinned dictation-gpui exe staged under the packaged name", () => {
  const pins = JSON.parse(
    readFileSync(path.join(import.meta.dirname, "..", "component-pins.json"), "utf8"),
  );
  expect(pins.dictation_gpui.repository).toBe("makekosmos/dictation");
  expect(pins.dictation_gpui.commit).toMatch(/^[0-9a-f]{40}$/);
  expect(source).toContain("KOSMOS_DICTATION_GPUI_SRC");
  expect(source).toContain("dictation-gpui");
  expect(source).toContain("KOSMOS_DICTATION_VERSION");
  expect(source).toContain('"Kosmos Dictation.exe"');
});
