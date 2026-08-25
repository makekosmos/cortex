import path from "node:path";

type AppBrand = { name: string; icon: string };
const brands = {
  "com.kosmos.memoria": { name: "Memoria", icon: "memoria.png" },
  "com.kosmos.agenda": { name: "Agenda", icon: "agenda.png" },
  "com.kosmos.arcadia": { name: "Arcadia", icon: "arcadia.png" },
  arcadia: { name: "Arcadia", icon: "arcadia.png" },
  arrancador: { name: "Arcadia", icon: "arcadia.png" },
  "com.kosmos.shell": { name: "Kosmos Shell", icon: "kosmos.png" },
  shell: { name: "Kosmos Shell", icon: "kosmos.png" },
  "com.kosmos.dictation": { name: "Dictation", icon: "dictation.png" },
  dictation: { name: "Dictation", icon: "dictation.png" },
} satisfies Record<string, AppBrand>;

const appBrand = (id: string): AppBrand | undefined =>
  Object.entries(brands).find(([key]) => key === id)?.[1];

export function kosmosAppName(id: string, fallback: string): string {
  return appBrand(id)?.name ?? fallback;
}

export function kosmosAppIcon(resourcesPath: string, id: string): string | undefined {
  const icon = appBrand(id)?.icon;
  return icon ? path.join(resourcesPath, "app-icons", icon) : undefined;
}

export function kosmosAppShortcutIcon(resourcesPath: string, id: string): string | undefined {
  const icon = appBrand(id)?.icon?.replace(/\.png$/u, ".ico");
  return icon ? path.join(resourcesPath, "app-icons", icon) : undefined;
}
