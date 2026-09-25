import path from "node:path";
import { randomUUID } from "node:crypto";
import { redactText as redactSharedText } from "../scripts/redaction.mjs";
import type { JsonRecord, JsonValue } from "./json-types";
export type { JsonRecord } from "./json-types";

export const MAX_SUPPORT_TEXT_FILE_BYTES = 1024 * 1024;
const SUPPORT_TEXT_EXTENSIONS = new Set([".json", ".log", ".txt"]);
const SENSITIVE_KEYS = new Set([
  "apikey",
  "authorization",
  "authsecret",
  "authtoken",
  "body",
  "content",
  "cookie",
  "data",
  "dbpath",
  "password",
  "payload",
  "request",
  "response",
  "secret",
  "text",
  "token",
]);

interface RedactableObject {}
type RedactableValue = JsonValue | RedactableObject;

export function redactText(value: string): string {
  return redactSharedText(value);
}

export function redactUnknown(value: RedactableValue, key = "", depth = 0): JsonValue {
  if (SENSITIVE_KEYS.has(normalizeKey(key))) return "[REDACTED]";
  if (isJsonString(value)) return redactText(value);
  if (value === null || !isJsonObject(value)) {
    // SAFETY: Primitive inputs are already members of the JSON value contract.
    return value as JsonValue;
  }
  if (depth >= 6) return "[TRUNCATED]";
  if (Array.isArray(value)) {
    return value.slice(0, 50).map((item) => redactUnknown(item, "", depth + 1));
  }
  return Object.fromEntries(
    Object.entries(value)
      .slice(0, 50)
      .map(([childKey, childValue]) => [childKey, redactUnknown(childValue, childKey, depth + 1)]),
  );
}

export function isSupportTextFile(fileName: string): boolean {
  return SUPPORT_TEXT_EXTENSIONS.has(path.extname(fileName).toLowerCase());
}

export function redactTextFile(contents: string): string {
  const bounded = Buffer.from(contents, "utf8").subarray(0, MAX_SUPPORT_TEXT_FILE_BYTES);
  return bounded
    .toString("utf8")
    .split(/\r?\n/)
    .map((line) => {
      try {
        return JSON.stringify(redactUnknown(JSON.parse(line)));
      } catch {
        return redactText(line);
      }
    })
    .join("\n");
}

export function createCrashMetadata(
  component: string,
  correlationId: string,
  meta?: RedactableObject,
): JsonRecord {
  const redacted = redactUnknown({
    ...meta,
    component,
    correlationId,
    crashId: randomUUID(),
  });
  return isJsonObject(redacted) ? redacted : {};
}

function normalizeKey(key: string): string {
  return key.toLowerCase().replace(/[^a-z]/g, "");
}

function isJsonString(value: RedactableValue): value is string {
  return typeof value === "string";
}

function isJsonObject(value: RedactableValue): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
