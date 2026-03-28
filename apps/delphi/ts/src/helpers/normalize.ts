/** Strip trailing slashes and whitespace from a URL. */
export function normalizeApiUrl(value: string): string {
  return value.trim().replace(/\/+$/, "");
}

/** Lowercase, collapse whitespace, trim a passphrase. */
export function normalizePassphrase(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, " ");
}
