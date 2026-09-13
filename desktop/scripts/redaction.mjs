const MAX_LOG_TEXT_BYTES = 16 * 1024;

export function redactText(value) {
  let output = String(value)
    .replace(/\bBearer\s+[A-Za-z0-9._~+/=-]+/gi, "Bearer [REDACTED]")
    .replace(
      /\b(?:authorization|auth[_-]?token|api[_-]?key|secret|password)\s*[:=]\s*[^\s,;]+/gi,
      "[REDACTED]",
    )
    .replace(/\b[0-9a-f]{64}\b/gi, "[REDACTED]")
    .replace(/\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g, "[REDACTED]")
    .replace(/[A-Z]:\\Users\\[^\s"'\\]+(?:\\[^\s"']+)*/gi, "[REDACTED_PATH]")
    .replace(/(?:\/Users|\/home)\/[^\s"']+/g, "[REDACTED_PATH]");
  const suffix = "\n[TRUNCATED]";
  if (Buffer.byteLength(output, "utf8") > MAX_LOG_TEXT_BYTES)
    output = `${Buffer.from(output, "utf8")
      .subarray(0, MAX_LOG_TEXT_BYTES - Buffer.byteLength(suffix))
      .toString("utf8")}${suffix}`;
  return output;
}
