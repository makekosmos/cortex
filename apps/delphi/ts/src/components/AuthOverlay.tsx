import { useState } from 'react';
import { getApiUrl } from '@/services/api/client';

type AuthOverlayProps = {
  busy: boolean;
  errorMessage: string | null;
  onSubmit: (input: { apiUrl: string; passphrase: string }) => Promise<void>;
};

export default function AuthOverlay({ busy, errorMessage, onSubmit }: AuthOverlayProps) {
  const [apiUrl, setApiUrl] = useState(getApiUrl());
  const [passphrase, setPassphrase] = useState('');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4">
      <div className="bg-background border-border w-full max-w-lg rounded-xl border p-5">
        <h2 className="mb-2 text-lg font-semibold">Connect to Delphi Server</h2>
        <p className="text-muted-foreground mb-4 text-sm">
          Enter API URL and your 12-word passphrase to enable sync.
        </p>

        <label htmlFor="auth-api-url" className="mb-2 block text-sm font-medium">
          API URL
        </label>
        <input
          id="auth-api-url"
          value={apiUrl}
          onChange={(event) => setApiUrl(event.target.value)}
          className="border-border bg-secondary mb-4 w-full rounded-md border px-3 py-2 text-sm"
          placeholder="http://213.165.58.219:3000"
          autoComplete="off"
        />

        <label htmlFor="auth-passphrase" className="mb-2 block text-sm font-medium">
          Passphrase
        </label>
        <input
          id="auth-passphrase"
          value={passphrase}
          onChange={(event) => setPassphrase(event.target.value)}
          className="border-border bg-secondary mb-4 w-full rounded-md border px-3 py-2 text-sm"
          placeholder="12 words separated by spaces"
          autoComplete="off"
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !busy) {
              void onSubmit({ apiUrl, passphrase });
            }
          }}
        />

        <button
          type="button"
          disabled={busy}
          onClick={() => {
            void onSubmit({ apiUrl, passphrase });
          }}
          className="bg-primary text-primary-foreground w-full rounded-md px-3 py-2 text-sm disabled:opacity-60"
        >
          {busy ? 'Connecting...' : 'Connect'}
        </button>

        {errorMessage ? <p className="text-destructive mt-3 text-sm">{errorMessage}</p> : null}
      </div>
    </div>
  );
}
