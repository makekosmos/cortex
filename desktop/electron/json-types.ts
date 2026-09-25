// JSON value contract shared by Electron main-side IPC handlers.
//
// Extracted from the legacy extension runtime (extension-permissions.ts):
// the .kext permission assertions are gone, but these plain-JSON types and
// guards are still used by IPC handlers, migration code, and logging.

type JsonPrimitive = string | number | boolean | null;

export interface JsonRecord {
  [key: string]: JsonValue;
}

export type JsonValue = JsonPrimitive | JsonValue[] | JsonRecord;

export function isRecord(value: JsonValue | undefined): value is JsonRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function isString(value: JsonValue | undefined): value is string {
  return typeof value === "string";
}
