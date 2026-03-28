import { useState } from "react";

type AuthOverlayProps = {
  busy: boolean;
  errorMessage: string | null;
  onSubmit: (connectionCode: string) => Promise<void>;
};

export default function AuthOverlay({
  busy,
  errorMessage,
  onSubmit,
}: AuthOverlayProps) {
  const [connectionCode, setConnectionCode] = useState("");

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4">
      <div className="bg-(--background) border-(--border) w-full max-w-lg rounded-xl border p-5">
        <h2 className="mb-2 text-lg font-semibold">Подключение к Ark</h2>
        <p className="text-(--muted-foreground) mb-4 text-sm">
          Вставьте код подключения, чтобы синхронизировать задачи через Ark.
        </p>

        <label
          htmlFor="auth-connection-code"
          className="mb-2 block text-sm font-medium"
        >
          Код подключения
        </label>
        <input
          id="auth-connection-code"
          value={connectionCode}
          onChange={(event) => setConnectionCode(event.target.value)}
          className="border-(--border) bg-(--secondary) mb-4 w-full rounded-md border px-3 py-2 text-sm"
          placeholder="ark://192.168.1.5:8000?key=..."
          autoComplete="off"
          onKeyDown={(event) => {
            if (event.key === "Enter" && !busy) {
              void onSubmit(connectionCode);
            }
          }}
        />

        <button
          type="button"
          disabled={busy}
          onClick={() => {
            void onSubmit(connectionCode);
          }}
          className="bg-(--primary) text-(--primary-foreground) w-full rounded-md px-3 py-2 text-sm disabled:opacity-60"
        >
          {busy ? "Подключение..." : "Подключить"}
        </button>

        {errorMessage ? (
          <p className="text-(--destructive) mt-3 text-sm">{errorMessage}</p>
        ) : null}
      </div>
    </div>
  );
}
