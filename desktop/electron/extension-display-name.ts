const LEGACY_EXTENSION_NAMES: Record<string, string> = {
  eden: "Memoria",
  delphi: "Agenda",
  arrancador: "Arcadia",
};

export const extensionDisplayName = (id: string, fallback: string): string =>
  LEGACY_EXTENSION_NAMES[id] ?? fallback;
