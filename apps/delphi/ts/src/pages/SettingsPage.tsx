import { Database, Link, Loader2, LogOut, Save, ShieldCheck, ShieldOff, SunMoon, Unlink } from 'lucide-react';
import { useState } from 'react';
import { useTheme } from '@/features/themeProvider';
import {
  clearSavedPassphrase,
  clearToken,
  getApiUrl,
  getReadableError,
  getToken,
  loginWithPassphrase,
  normalizeApiUrl,
  normalizePassphrase,
  setApiUrl,
} from '@/services/api/client';
import {
  arkSync,
  getArkApiKey,
  getArkUrl,
  setArkApiKey,
  setArkUrl,
} from '@/services/sync/ark-client';
import { claimPairingCode } from '@/services/sync/pairing';

export default function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const [apiUrlDraft, setApiUrlDraft] = useState(getApiUrl());
  const [passphrase, setPassphrase] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [hasToken, setHasToken] = useState(Boolean(getToken()));

  const isPaired = Boolean(getArkUrl() && getArkApiKey());
  const [arkConnected, setArkConnected] = useState(arkSync.isConnected);
  const [arkMessage, setArkMessage] = useState('');

  // Pairing form state (when not paired)
  const [pairingServerUrl, setPairingServerUrl] = useState('');
  const [pairingCode, setPairingCode] = useState('');
  const [pairing, setPairing] = useState(false);
  const [paired, setPaired] = useState(isPaired);

  const handlePair = async () => {
    const serverUrl = pairingServerUrl.trim().replace(/\/+$/, '');
    const code = pairingCode.trim();

    if (!serverUrl || !code) {
      setArkMessage('Укажите сервер и код сопряжения.');
      return;
    }

    setPairing(true);
    setArkMessage('');

    try {
      const result = await claimPairingCode(serverUrl, code, 'Delphi Web');
      setArkUrl(result.server_url);
      setArkApiKey(result.api_key);
      setPaired(true);

      // Auto-connect after pairing
      arkSync.disconnect();
      arkSync.onStatus((connected) => setArkConnected(connected));
      arkSync.connect(result.server_url, result.api_key);
      setArkMessage('Устройство привязано.');
    } catch (error) {
      setArkMessage(error instanceof Error ? error.message : 'Ошибка привязки');
    } finally {
      setPairing(false);
    }
  };

  const handleUnpair = () => {
    arkSync.disconnect();
    setArkUrl('');
    setArkApiKey('');
    setArkConnected(false);
    setPaired(false);
    setArkMessage('Устройство отвязано.');
  };

  const handleReconnectArk = () => {
    const url = getArkUrl();
    const key = getArkApiKey();
    if (!url || !key) return;

    arkSync.disconnect();
    arkSync.onStatus((connected) => setArkConnected(connected));
    arkSync.connect(url, key);
    setArkMessage('Подключение...');
  };

  const handleDisconnectArk = () => {
    arkSync.disconnect();
    setArkConnected(false);
    setArkMessage('Отключено от Ark.');
  };

  const handleSaveApiUrl = () => {
    const normalized = normalizeApiUrl(apiUrlDraft);
    if (!normalized) {
      setMessage('API URL is required.');
      return;
    }

    setApiUrl(normalized);
    clearToken();
    setHasToken(false);
    setMessage('API URL saved. Sign in again to continue syncing.');
  };

  const handleLogin = async () => {
    const normalizedPassphrase = normalizePassphrase(passphrase);
    if (!normalizedPassphrase) {
      setMessage('Passphrase is required.');
      return;
    }

    setBusy(true);
    setMessage('');
    try {
      await loginWithPassphrase(normalizedPassphrase);
      setHasToken(true);
      setPassphrase('');
      setMessage('Connected. Tasks will sync with server.');
    } catch (error) {
      setMessage(getReadableError(error));
    } finally {
      setBusy(false);
    }
  };

  const handleLogout = () => {
    clearToken();
    clearSavedPassphrase();
    setHasToken(false);
    setMessage('Signed out. Local cache is still available.');
  };

  return (
    <div className="h-full min-h-0 w-full overflow-auto bg-black/55 p-4">
      <div className="mx-auto flex w-full max-w-lg flex-col gap-4 py-6">
        <section className="bg-background border-border w-full rounded-xl border p-5">
          <h1 className="mb-2 text-lg font-semibold">Settings</h1>
          <p className="text-muted-foreground mb-4 text-sm">
            Configure server connection and sign in.
          </p>

          <label className="mb-2 block text-sm font-medium" htmlFor="api-url">
            API URL
          </label>
          <div className="mb-4 flex gap-2">
            <input
              id="api-url"
              value={apiUrlDraft}
              onChange={(event) => setApiUrlDraft(event.target.value)}
              className="border-border bg-secondary flex-1 rounded-md border px-3 py-2 text-sm"
              placeholder="https://213.165.58.219.nip.io"
              autoComplete="off"
            />
            <button
              type="button"
              onClick={handleSaveApiUrl}
              className="bg-primary text-primary-foreground inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
            >
              <Save size={16} />
              Save
            </button>
          </div>

          <label className="mb-2 block text-sm font-medium" htmlFor="passphrase">
            Passphrase
          </label>
          <div className="mb-3 flex gap-2">
            <input
              id="passphrase"
              value={passphrase}
              onChange={(event) => setPassphrase(event.target.value)}
              className="border-border bg-secondary flex-1 rounded-md border px-3 py-2 text-sm"
              placeholder="12 words separated by spaces"
              autoComplete="off"
            />
            <button
              type="button"
              onClick={() => {
                void handleLogin();
              }}
              disabled={busy}
              className="bg-primary text-primary-foreground inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm disabled:opacity-60"
            >
              <ShieldCheck size={16} />
              {busy ? 'Signing in...' : 'Sign in'}
            </button>
          </div>

          <div className="flex items-center justify-between text-sm">
            <div className="text-muted-foreground inline-flex items-center gap-2">
              {hasToken ? <ShieldCheck size={16} /> : <ShieldOff size={16} />}
              {hasToken ? 'Authenticated' : 'Not authenticated'}
            </div>
            <button
              type="button"
              onClick={handleLogout}
              className="text-destructive inline-flex items-center gap-2"
            >
              <LogOut size={16} />
              Sign out
            </button>
          </div>

          {message ? <p className="text-muted-foreground mt-3 text-sm">{message}</p> : null}
        </section>

        <section className="bg-background border-border w-full rounded-xl border p-5">
          <h2 className="mb-3 inline-flex items-center gap-2 text-base font-semibold">
            <Database size={16} />
            Ark Server
          </h2>
          <p className="text-muted-foreground mb-4 text-sm">
            Синхронизация между устройствами через Ark.
          </p>

          {paired ? (
            <>
              <div className="mb-4 flex items-center gap-2 text-sm">
                <div
                  className={`h-2 w-2 rounded-full ${arkConnected ? 'bg-emerald-500' : 'bg-rose-500'}`}
                />
                <span className="text-muted-foreground">
                  {arkConnected ? 'Подключено' : 'Отключено'}
                </span>
              </div>

              <label className="mb-2 block text-sm font-medium">
                Сервер
              </label>
              <input
                value={getArkUrl()}
                readOnly
                className="border-border bg-secondary text-muted-foreground mb-4 w-full rounded-md border px-3 py-2 text-sm font-mono"
              />

              <div className="flex items-center gap-2">
                {arkConnected ? (
                  <button
                    type="button"
                    onClick={handleDisconnectArk}
                    className="bg-secondary text-secondary-foreground inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
                  >
                    Отключиться
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={handleReconnectArk}
                    className="bg-primary text-primary-foreground inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
                  >
                    Подключиться
                  </button>
                )}
                <button
                  type="button"
                  onClick={handleUnpair}
                  className="text-destructive inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
                >
                  <Unlink size={16} />
                  Отвязать
                </button>
              </div>
            </>
          ) : (
            <>
              <label className="mb-2 block text-sm font-medium" htmlFor="pairing-server">
                Сервер
              </label>
              <input
                id="pairing-server"
                value={pairingServerUrl}
                onChange={(event) => setPairingServerUrl(event.target.value)}
                className="border-border bg-secondary mb-4 w-full rounded-md border px-3 py-2 text-sm"
                placeholder="https://your-ark-server.com"
                autoComplete="off"
                autoCapitalize="off"
              />

              <label className="mb-2 block text-sm font-medium" htmlFor="pairing-code">
                Код сопряжения
              </label>
              <input
                id="pairing-code"
                value={pairingCode}
                onChange={(event) => setPairingCode(event.target.value)}
                className="border-border bg-secondary mb-4 w-full rounded-md border px-3 py-2 text-sm font-mono"
                placeholder="ark-XXXX"
                autoComplete="off"
                autoCapitalize="off"
              />

              <button
                type="button"
                onClick={() => { void handlePair(); }}
                disabled={pairing}
                className="bg-primary text-primary-foreground inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm disabled:opacity-60"
              >
                {pairing ? (
                  <>
                    <Loader2 size={16} className="animate-spin" />
                    Привязка...
                  </>
                ) : (
                  <>
                    <Link size={16} />
                    Привязать
                  </>
                )}
              </button>
            </>
          )}

          {arkMessage ? (
            <p className="text-muted-foreground mt-3 text-sm">{arkMessage}</p>
          ) : null}
        </section>

        <section className="bg-background border-border w-full rounded-xl border p-5">
          <h2 className="mb-3 inline-flex items-center gap-2 text-base font-semibold">
            <SunMoon size={16} />
            Theme
          </h2>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => setTheme('light')}
              className={`rounded-md px-3 py-2 text-sm ${
                theme === 'light' ? 'bg-primary text-primary-foreground' : 'bg-secondary'
              }`}
            >
              Light
            </button>
            <button
              type="button"
              onClick={() => setTheme('dark')}
              className={`rounded-md px-3 py-2 text-sm ${
                theme === 'dark' ? 'bg-primary text-primary-foreground' : 'bg-secondary'
              }`}
            >
              Dark
            </button>
            <button
              type="button"
              onClick={() => setTheme('system')}
              className={`rounded-md px-3 py-2 text-sm ${
                theme === 'system' ? 'bg-primary text-primary-foreground' : 'bg-secondary'
              }`}
            >
              System
            </button>
          </div>
        </section>
      </div>
    </div>
  );
}
