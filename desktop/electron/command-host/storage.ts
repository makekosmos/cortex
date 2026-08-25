import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { isRecord, isString, type CommandRecord } from "./view-model";

interface StringMap {
  [key: string]: string;
}

export interface CommandStorage {
  get(key: string): string | undefined;
  all(): StringMap;
  set(key: string, value: string): void;
  remove(key: string): void;
  clear(): void;
}

function toStringMap(value: CommandRecord): StringMap {
  const result: StringMap = {};
  for (const [key, item] of Object.entries(value)) {
    if (isString(item)) result[key] = item;
  }
  return result;
}

function readJsonFile(filePath: string): StringMap {
  if (!existsSync(filePath)) return {};
  try {
    // SAFETY: JSON.parse is constrained to the command storage value grammar below.
    const parsed = JSON.parse(readFileSync(filePath, "utf8")) as CommandRecord;
    return parsed && isRecord(parsed) && !Array.isArray(parsed) ? toStringMap(parsed) : {};
  } catch {
    return {};
  }
}

function writeJsonFile(filePath: string, value: StringMap): void {
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, JSON.stringify(value, null, 2), "utf8");
}

export function createStorage(filePath: string): CommandStorage {
  return {
    get(key) {
      return readJsonFile(filePath)[key];
    },
    all() {
      return { ...readJsonFile(filePath) };
    },
    set(key, value) {
      const data = readJsonFile(filePath);
      data[key] = value;
      writeJsonFile(filePath, data);
    },
    remove(key) {
      const data = readJsonFile(filePath);
      delete data[key];
      writeJsonFile(filePath, data);
    },
    clear() {
      writeJsonFile(filePath, {});
    },
  };
}

export function createCacheStorage(filePath: string, namespace: string): CommandStorage {
  return createStorage(path.join(filePath, `${namespace}.json`));
}
