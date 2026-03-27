import { Database, LogOut, Save, ShieldCheck, ShieldOff, SunMoon } from 'lucide-react';
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

export default function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const [apiUrlDraft, setApiUrlDraft] = useState(getApiUrl());
  const [passphrase, setPassphrase] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [hasToken, setHasToken] = useState(Boolean(getToken()));

  const [arkUrlDraft, setArkUrlDraft] = useState(getArkUrl());
  const [arkKeyDraft, setArkKeyDraft] = useState(getArkApiKey());
  const [arkMessage, setArkMessage] = useState('');
  const [arkConnected, setArkConnected] = useState(arkSync.isConnected);

  const handleSaveArk = () => {
    const url = arkUrlDraft.trim().replace(/\/+$/, '');
    const key = arkKeyDraft.trim();

    if (!url || !key) {
      setArkMessage('Both Ark URL and API key are required.');
      return;
    }

    setArkUrl(url);
    setArkApiKey(key);

    arkSync.disconnect();
    arkSync.onStatus((connected) => setArkConnected(connected));
    arkSync.connect(url, key);
    setArkMessage('Connecting to Ark...');
  };

  const handleDisconnectArk = () => {
    arkSync.disconnect();
    setArkConnected(false);
    setArkMessage('Disconnected from Ark.');
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
            Connect to Ark for cross-device sync.
          </p>

          <label className="mb-2 block text-sm font-medium" htmlFor="ark-url">
            Server URL
          </label>
          <input
            id="ark-url"
            value={arkUrlDraft}
            onChange={(event) => setArkUrlDraft(event.target.value)}
            className="border-border bg-secondary mb-4 w-full rounded-md border px-3 py-2 text-sm"
            placeholder="https://your-ark-server.com"
            autoComplete="off"
          />

          <label className="mb-2 block text-sm font-medium" htmlFor="ark-key">
            API Key
          </label>
          <input
            id="ark-key"
            value={arkKeyDraft}
            onChange={(event) => setArkKeyDraft(event.target.value)}
            className="border-border bg-secondary mb-4 w-full rounded-md border px-3 py-2 text-sm font-mono"
            placeholder="your-api-key"
            autoComplete="off"
            type="password"
          />

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleSaveArk}
              className="bg-primary text-primary-foreground inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
            >
              <Save size={16} />
              {arkConnected ? 'Reconnect' : 'Connect'}
            </button>
            {arkConnected ? (
              <button
                type="button"
                onClick={handleDisconnectArk}
                className="text-destructive inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
              >
                Disconnect
              </button>
            ) : null}
          </div>

          <div className="mt-3 flex items-center gap-2 text-sm">
            <div
              className={`h-2 w-2 rounded-full ${arkConnected ? 'bg-emerald-500' : 'bg-rose-500'}`}
            />
            <span className="text-muted-foreground">
              {arkConnected ? 'Connected to Ark' : 'Not connected'}
            </span>
          </div>

          {arkMessage ? (
            <p className="text-muted-foreground mt-2 text-sm">{arkMessage}</p>
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
