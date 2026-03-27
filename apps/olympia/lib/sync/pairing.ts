/**
 * Ark server pairing -- claim a pairing code to get credentials.
 *
 * POST /pairing/claim { code, device_name, platform }
 *   -> { server_url, api_key, device_id }
 */

import { Platform } from 'react-native';

export interface PairingResult {
  server_url: string;
  api_key: string;
  device_id: string;
}

/**
 * Claim a pairing code against the Ark server.
 *
 * @param serverBaseUrl  Base URL of the Ark server (e.g. "https://ark.myserver.com")
 * @param code           8-char pairing code (e.g. "ark-7f3k")
 * @param deviceName     Human-readable device name
 */
export async function claimPairingCode(
  serverBaseUrl: string,
  code: string,
  deviceName: string,
): Promise<PairingResult> {
  const base = serverBaseUrl.replace(/\/+$/, '');
  const url = `${base}/pairing/claim`;

  const res = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      code,
      device_name: deviceName,
      platform: Platform.OS,
    }),
  });

  if (!res.ok) {
    const body = await res.text().catch(() => '');
    if (res.status === 404) throw new Error('Код не найден или истёк');
    if (res.status === 409) throw new Error('Код уже использован');
    throw new Error(`Ошибка сервера (${res.status}): ${body}`);
  }

  const data = (await res.json()) as PairingResult;

  if (!data.server_url || !data.api_key || !data.device_id) {
    throw new Error('Некорректный ответ сервера');
  }

  return data;
}
