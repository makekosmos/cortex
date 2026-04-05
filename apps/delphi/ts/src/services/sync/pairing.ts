/**
 * Ark connection string parser.
 *
 * Format: ark://host:port?key=SECRET
 * Contains everything needed to connect — no intermediate claim step.
 */

export interface ArkConnection {
  server_url: string;

  api_key: string;
}

/**
 * Parse an Ark connection string into server URL and API key.
 *
 * @param connectionString  e.g. "ark://192.168.1.5:8000?key=dev-test-key"
 * @returns parsed connection or null if invalid format
 */

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
