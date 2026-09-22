const DAY_MS = 24 * 60 * 60 * 1000;

export class FatSecretError extends Error {
  constructor(kind, message) {
    super(message);
    this.name = "FatSecretError";
    this.kind = kind;
  }
}

function validIsoDate(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const time = Date.parse(`${value}T00:00:00Z`);
  return Number.isFinite(time) && new Date(time).toISOString().slice(0, 10) === value;
}

export function dateIntToIso(value) {
  const text = String(value ?? "").trim();
  const days = Number(text);
  if (!/^-?\d+$/.test(text) || !Number.isSafeInteger(days))
    throw new FatSecretError("api", "FatSecret food entry date_int is invalid");
  const date = new Date(days * DAY_MS);
  if (Number.isNaN(date.getTime()))
    throw new FatSecretError("api", "FatSecret food entry date_int is invalid");
  return date.toISOString().slice(0, 10);
}

export function normalizeDate(value) {
  const text = String(value ?? "");
  if (/^-?\d+$/.test(text.trim())) return dateIntToIso(text);
  if (validIsoDate(text)) return text;
  throw new FatSecretError("api", "FatSecret food entry date is invalid");
}

export function dateRange(from, to) {
  const end = to ?? from ?? new Date().toISOString().slice(0, 10);
  const start = from ?? end;
  if (!validIsoDate(start) || !validIsoDate(end) || start > end)
    throw new FatSecretError("config", "FatSecret sync date range is invalid");
  const startMs = Date.parse(`${start}T00:00:00Z`);
  const endMs = Date.parse(`${end}T00:00:00Z`);
  const count = Math.floor((endMs - startMs) / DAY_MS) + 1;
  if (count > 31) throw new FatSecretError("config", "FatSecret sync range cannot exceed 31 days");
  return Array.from({ length: count }, (_, index) =>
    new Date(startMs + index * DAY_MS).toISOString().slice(0, 10),
  );
}

export function utcEpochDays(isoDate) {
  return Math.floor(Date.parse(`${isoDate}T00:00:00Z`) / DAY_MS);
}

export function parseResponse(response) {
  if (!response || typeof response.status !== "number" || typeof response.body !== "string")
    throw new FatSecretError("invalid_response", "FatSecret response is invalid");
  if (response.status !== 200) {
    const kind =
      response?.status === 401 ? "auth" : response?.status === 429 ? "rate_limit" : "api";
    const message =
      response?.status === 401
        ? "FatSecret authorization failed"
        : response?.status === 429
          ? "FatSecret rate limit reached"
          : "FatSecret API request failed";
    throw new FatSecretError(kind, message);
  }
  return response.body;
}

export function foodEntries(body) {
  let parsed;
  try {
    parsed = JSON.parse(body);
  } catch {
    throw new FatSecretError("invalid_response", "FatSecret response body is invalid");
  }
  if (
    !parsed?.food_entries ||
    typeof parsed.food_entries !== "object" ||
    Array.isArray(parsed.food_entries)
  )
    throw new FatSecretError("invalid_response", "FatSecret response body is invalid");
  const entries = parsed.food_entries.food_entry;
  if (entries === undefined || entries === null) return [];
  if (Array.isArray(entries)) return entries;
  if (typeof entries === "object") return [entries];
  throw new FatSecretError("invalid_response", "FatSecret response body is invalid");
}
