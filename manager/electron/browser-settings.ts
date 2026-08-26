import fs from "node:fs";
import path from "node:path";

export function readBrowserDataPersistence(file: string): boolean {
  try {
    const value = JSON.parse(fs.readFileSync(file, "utf8"));
    return value?.persistData !== false;
  } catch {
    return true;
  }
}

export function writeBrowserDataPersistence(file: string, enabled: boolean) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify({ persistData: enabled }), "utf8");
}

export function browserPartition(name: string, persistent: boolean): string {
  return persistent ? `persist:${name}` : `${name}-private`;
}
