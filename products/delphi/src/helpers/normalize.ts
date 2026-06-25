/** Strip trailing slashes and whitespace from a URL. */

export function normalizeApiUrl(value: string): string {
  return value.trim().replace(/\/+$/, "");
}
