import path from "node:path";
import { randomUUID } from "node:crypto";

const MAX_LOG_TEXT_BYTES = 16 * 1024;
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

export function redactText(value: string): string {
  let output = value
    .replace(/\bBearer\s+[A-Za-z0-9._~+/=-]+/gi, "Bearer [REDACTED]")
    .replace(
      /\b(?:authorization|auth[_-]?token|api[_-]?key|secret|password)\s*[:=]\s*[^\s,;]+/gi,
      "[REDACTED]",
    )
    .replace(/\b[0-9a-f]{64}\b/gi, "[REDACTED]")
    .replace(/\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g, "[REDACTED]")
    .replace(/[A-Z]:\\Users\\[^\s"'\\]+(?:\\[^\s"']+)*/gi, "[REDACTED_PATH]")
    .replace(/(?:\/Users|\/home)\/[^\s"']+/g, "[REDACTED_PATH]");
  if (Buffer.byteLength(output, "utf8") > MAX_LOG_TEXT_BYTES) {
    output = Buffer.from(output, "utf8").subarray(0, MAX_LOG_TEXT_BYTES).toString("utf8");
    output += "[TRUNCATED]";
  }
  return output;
}

export function redactUnknown(value: unknown, key = "", depth = 0): unknown {
  if (SENSITIVE_KEYS.has(normalizeKey(key))) return "[REDACTED]";
  if (typeof value === "string") return redactText(value);
  if (value === null || typeof value !== "object") return value;
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
  meta?: object,
): Record<string, unknown> {
  return redactUnknown({
    ...meta,
    component,
    correlationId,
    crashId: randomUUID(),
  }) as Record<string, unknown>;
}

function normalizeKey(key: string): string {
  return key.toLowerCase().replace(/[^a-z]/g, "");
}
