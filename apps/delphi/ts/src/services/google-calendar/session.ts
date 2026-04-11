export const TOKEN_REFRESH_SKEW_MS = 60_000;

export function isAccessTokenExpired(
  expiresAt: string | null | undefined,
  now = Date.now(),
  skewMs = TOKEN_REFRESH_SKEW_MS,
): boolean {
  if (!expiresAt) return true;
  const expiry = Date.parse(expiresAt);
  if (!Number.isFinite(expiry)) return true;
  return expiry - skewMs <= now;
}

export function canRestoreGoogleSession(params: {
  refreshToken?: string | null;
  clientId?: string | null;
}): boolean {
  return Boolean(params.refreshToken?.trim() && params.clientId?.trim());
}
