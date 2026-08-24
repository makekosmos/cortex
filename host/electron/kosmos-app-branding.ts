import path from "node:path";

const brands: Record<string, { name: string; icon: string }> = {
  "com.kosmos.eden": { name: "Memoria", icon: "memoria.png" },
  "com.kosmos.memoria": { name: "Memoria", icon: "memoria.png" },
  eden: { name: "Memoria", icon: "memoria.png" },
  "com.kosmos.delphi": { name: "Agenda", icon: "agenda.png" },
  "com.kosmos.agenda": { name: "Agenda", icon: "agenda.png" },
  delphi: { name: "Agenda", icon: "agenda.png" },
  "com.kosmos.arcadia": { name: "Arcadia", icon: "arcadia.png" },
  arcadia: { name: "Arcadia", icon: "arcadia.png" },
  arrancador: { name: "Arcadia", icon: "arcadia.png" },
  "com.kosmos.shell": { name: "Kosmos Shell", icon: "kosmos.png" },
  shell: { name: "Kosmos Shell", icon: "kosmos.png" },
  "com.kosmos.dictation": { name: "Dictation", icon: "dictation.png" },
  dictation: { name: "Dictation", icon: "dictation.png" },
};

export function kosmosAppName(id: string, fallback: string): string {
  return brands[id]?.name ?? fallback;
}

export function kosmosAppIcon(resourcesPath: string, id: string): string | undefined {
  const icon = brands[id]?.icon;
  return icon ? path.join(resourcesPath, "app-icons", icon) : undefined;
}

export function kosmosAppShortcutIcon(resourcesPath: string, id: string): string | undefined {
  const icon = brands[id]?.icon?.replace(/\.png$/u, ".ico");
  return icon ? path.join(resourcesPath, "app-icons", icon) : undefined;
}
