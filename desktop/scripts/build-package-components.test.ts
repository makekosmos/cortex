import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";

test("packaged Manager and Host inherit the Desktop release version", () => {
  const source = readFileSync(
    path.join(import.meta.dirname, "build-package-components.mjs"),
    "utf8",
  );
  const icons = readFileSync(path.join(import.meta.dirname, "build-app-icons.mjs"), "utf8");
  expect(source).toContain('release-versions.json"), "utf8"');
  expect(source).toContain("scripts/build-app-icons.mjs");
  expect(icons).toContain("png-to-ico");
  expect(source).toContain("--config.extraMetadata.version=${version}");
  expect(source).toContain("--config.win.signExecutable=false");
  expect(source).not.toContain("--config.win.signAndEditExecutable=false");
});

test("components/manager is the manager-gpui exe staged under the packaged name", () => {
  const source = readFileSync(
    path.join(import.meta.dirname, "build-package-components.mjs"),
    "utf8",
  );
  const resolver = readFileSync(
    path.join(import.meta.dirname, "..", "electron", "manager-navigation.ts"),
    "utf8",
  );
  const packaged = JSON.parse(
    readFileSync(path.join(import.meta.dirname, "..", "package.json"), "utf8"),
  );
  expect(source).toContain('"cargo"');
  expect(source).toContain('"--locked"');
  expect(source).toContain("x86_64-pc-windows-msvc");
  expect(source).toContain("manager-gpui");
  expect(source).toContain('"Kosmos Manager.exe"');
  expect(resolver).toContain('"components", "manager", "Kosmos Manager.exe"');
  expect(source).not.toContain('"manager", "host"');
  expect(
    packaged.build.win.extraResources.some(
      (entry: { from: string; to: string }) =>
        entry.from === ".tmp/components/manager/win-unpacked" && entry.to === "components/manager",
    ),
  ).toBeTruthy();
});

test("components/agenda is the pinned agenda-gpui exe staged under the packaged name", () => {
  const source = readFileSync(
    path.join(import.meta.dirname, "build-package-components.mjs"),
    "utf8",
  );
  const resolver = readFileSync(
    path.join(import.meta.dirname, "..", "electron", "agenda-navigation.ts"),
    "utf8",
  );
  const packaged = JSON.parse(
    readFileSync(path.join(import.meta.dirname, "..", "package.json"), "utf8"),
  );
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
  expect(resolver).toContain('"components", "agenda", "Kosmos Agenda.exe"');
  expect(resolver).toContain("KOSMOS_AGENDA_EXECUTABLE");
  expect(resolver).toContain("KOSMOS_DATA_DIR");
  expect(
    packaged.build.win.extraResources.some(
      (entry: { from: string; to: string }) =>
        entry.from === ".tmp/components/agenda/win-unpacked" && entry.to === "components/agenda",
    ),
  ).toBeTruthy();
});

test("components/memoria is the pinned memoria-gpui exe staged under the packaged name", () => {
  const source = readFileSync(
    path.join(import.meta.dirname, "build-package-components.mjs"),
    "utf8",
  );
  const resolver = readFileSync(
    path.join(import.meta.dirname, "..", "electron", "memoria-navigation.ts"),
    "utf8",
  );
  const packaged = JSON.parse(
    readFileSync(path.join(import.meta.dirname, "..", "package.json"), "utf8"),
  );
  const pins = JSON.parse(
    readFileSync(path.join(import.meta.dirname, "..", "component-pins.json"), "utf8"),
  );
  expect(pins.memoria_gpui.repository).toBe("makekosmos/memoria-gpui");
  expect(pins.memoria_gpui.commit).toMatch(/^[0-9a-f]{40}$/);
  expect(source).toContain("KOSMOS_MEMORIA_GPUI_SRC");
  expect(source).toContain("memoria-gpui");
  expect(source).toContain("KOSMOS_MEMORIA_VERSION");
  expect(source).toContain('"Kosmos Memoria.exe"');
  expect(resolver).toContain('"components", "memoria", "Kosmos Memoria.exe"');
  expect(resolver).toContain("KOSMOS_MEMORIA_EXECUTABLE");
  expect(resolver).toContain("KOSMOS_DATA_DIR");
  expect(
    packaged.build.win.extraResources.some(
      (entry: { from: string; to: string }) =>
        entry.from === ".tmp/components/memoria/win-unpacked" && entry.to === "components/memoria",
    ),
  ).toBeTruthy();
});
