/**
 * Ark connection string parser.
 *
 * Format: ark://host:port?key=SECRET
 */

export interface ArkConnection {
  server_url: string;
  api_key: string;
}

export function parseConnectionString(
  connectionString: string,
): ArkConnection | null {
  const trimmed = connectionString.trim();
  if (!trimmed.startsWith("ark://")) return null;

  const rest = trimmed.slice("ark://".length);
  const keyIndex = rest.indexOf("?key=");
  if (keyIndex === -1) return null;

  const hostPart = rest.slice(0, keyIndex);
  const apiKey = rest.slice(keyIndex + "?key=".length);

  if (!hostPart || !apiKey) return null;

  return {
    server_url: `http://${hostPart}`,
    api_key: apiKey,
  };
}
