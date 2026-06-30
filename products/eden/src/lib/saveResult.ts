export function isFailedSaveResult(result: unknown): result is { ok: false } {
  return !!result && typeof result === "object" && "ok" in result && result.ok === false;
}
