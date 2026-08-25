const LEGACY_EXTENSION_NAMES = {
  eden: "Memoria",
  delphi: "Agenda",
  arrancador: "Arcadia",
} satisfies Record<string, string>;

export const extensionDisplayName = (id: string, fallback: string): string =>
  Object.entries(LEGACY_EXTENSION_NAMES).find(([legacyId]) => legacyId === id)?.[1] ?? fallback;
