export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonRecord | JsonValue[];

export interface JsonRecord {
  [key: string]: JsonValue | undefined;
}

export function isRecord<T>(value: T): value is T & JsonRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function isString<T>(value: T): value is T & string {
  return typeof value === "string";
}

export function isNumber<T>(value: T): value is T & number {
  return typeof value === "number";
}

export function parseJsonRecord<T>(value: T): T & JsonRecord {
  if (!isRecord(value)) throw new Error("Expected a JSON object");
  return value;
}
