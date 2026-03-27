import {
  buildTasksWsUrl,
  clearSavedPassphrase,
  clearToken,
  getApiUrl,
  getSavedPassphrase,
  getToken,
  loginWithPassphrase,
  normalizeApiUrl,
  normalizePassphrase,
  setApiUrl,
} from '@/services/api/client';

function clearAllCookies() {
  document.cookie.split(';').forEach((cookie) => {
    const [rawName] = cookie.split('=');
    const name = rawName?.trim();
    if (!name) return;
    document.cookie = `${name}=; path=/; max-age=0`;
  });
}

describe('api client helpers', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    clearAllCookies();
    vi.unstubAllGlobals();
  });

  it('normalizes API URLs', () => {
    expect(normalizeApiUrl(' http://example.com/// ')).toBe('http://example.com');
  });

  it('normalizes passphrases', () => {
    expect(normalizePassphrase('  WORD  one   TWO ')).toBe('word one two');
  });

  it('stores and resolves API URL', () => {
    setApiUrl('http://api.example.com///');
    expect(getApiUrl()).toBe('http://api.example.com');
  });

  it('builds websocket URL', () => {
    const wsUrl = buildTasksWsUrl('http://localhost:3000/', 'abc 123');
    expect(wsUrl).toBe('ws://localhost:3000/ws?token=abc%20123');
  });

  it('logs in and stores token + passphrase', async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ token: 'jwt-token', userId: 'u1' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    );

    vi.stubGlobal('fetch', fetchMock);

    await loginWithPassphrase('  WORD one TWO  ');

    expect(getToken()).toBe('jwt-token');
    expect(getSavedPassphrase()).toBe('word one two');

    clearToken();
    clearSavedPassphrase();

    expect(getToken()).toBeNull();
    expect(getSavedPassphrase()).toBeNull();
  });
});
