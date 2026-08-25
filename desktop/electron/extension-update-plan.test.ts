import { expect, test } from "bun:test";
import { findExtensionUpdates } from "./extension-update-plan";

test("updates match the immutable appId, not a display or extension id", () => {
  // SAFETY: Test fixtures satisfy the installed-extension contract.
  const installed = [
    {
      id: "eden",
      appId: "com.kosmos.memoria",
      name: "Memoria",
      version: "1.0.0",
      source: "installed",
    },
// SAFETY: The surrounding boundary establishes this documented contract.
  ] as never;
  const catalog = {
    extensions: [
      {
        id: "anything-can-be-renamed",
        appId: "com.kosmos.memoria",
        version: "1.0.1",
        downloadUrl: "https://example.com/memoria.kext",
        sha256: null,
      },
    ],
  };

  expect(findExtensionUpdates(installed, catalog)).toMatchObject([
    { id: "eden", nextVersion: "1.0.1" },
  ]);
});

test("legacy extensions without appId are never auto-updated by a name match", () => {
  // SAFETY: Test fixtures satisfy the installed-extension contract.
  const installed = [
    { id: "eden", appId: null, name: "Eden", version: "1.0.0", source: "installed" },
// SAFETY: The surrounding boundary establishes this documented contract.
  ] as never;
  const catalog = {
    extensions: [
      {
        id: "eden",
        appId: "com.kosmos.memoria",
        version: "1.0.1",
        downloadUrl: "https://example.com/eden.kext",
        sha256: null,
      },
    ],
  };

  expect(findExtensionUpdates(installed, catalog)).toEqual([]);
});

test("different appIds never match a catalog record", () => {
  // SAFETY: Test fixtures satisfy the installed-extension contract.
  const installed = [
    {
      id: "delphi",
      appId: "com.kosmos.unrelated",
      name: "Agenda",
      version: "1.0.0",
      source: "installed",
    },
// SAFETY: The surrounding boundary establishes this documented contract.
  ] as never;
  const catalog = {
    extensions: [
      {
        id: "any-repository-path",
        appId: "com.kosmos.agenda",
        version: "1.0.1",
        downloadUrl: "https://example.com/agenda.kext",
        sha256: null,
      },
    ],
  };

  expect(findExtensionUpdates(installed, catalog)).toEqual([]);
});
