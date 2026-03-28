import { Database, Link, SunMoon, Unlink } from "lucide-react";
import { useEffect, useState } from "react";
import { useTheme } from "@/features/themeProvider";
import {
  arkSync,
  getArkApiKey,
  getArkUrl,
  setArkApiKey,
  setArkUrl,
} from "@/services/sync/ark-client";
import { parseConnectionString } from "@/services/sync/pairing";

export default function SettingsPage() {
  const { theme, setTheme } = useTheme();

  const isPaired = Boolean(getArkUrl() && getArkApiKey());
  const [arkConnected, setArkConnected] = useState(arkSync.isConnected);
  const [arkMessage, setArkMessage] = useState("");

  // Pairing form state (when not paired)
  const [connectionCode, setConnectionCode] = useState("");
  const [paired, setPaired] = useState(isPaired);

  // Single status listener with cleanup — prevents memory leak from
  // registering a new listener on every connect/reconnect click.
  useEffect(() => {
    const unsub = arkSync.onStatus((connected) => setArkConnected(connected));
    return unsub;
  }, []);

  const handleConnect = () => {
    const parsed = parseConnectionString(connectionCode);
    if (!parsed) {
      setArkMessage("Неверный формат. Ожидается: ark://host:port?key=...");
      return;
    }

    setArkUrl(parsed.server_url);
    setArkApiKey(parsed.api_key);
    setPaired(true);

    // Auto-connect
    arkSync.disconnect();
    arkSync.connect(parsed.server_url, parsed.api_key);
    setArkMessage("Подключено к Ark.");
  };

  const handleUnpair = () => {
    arkSync.disconnect();
    setArkUrl("");
    setArkApiKey("");
    setArkConnected(false);
    setPaired(false);
    setArkMessage("Устройство отвязано.");
  };

  const handleReconnectArk = () => {
    const url = getArkUrl();
    const key = getArkApiKey();
    if (!url || !key) return;

    arkSync.disconnect();
    arkSync.connect(url, key);
    setArkMessage("Подключение...");
  };

  const handleDisconnectArk = () => {
    arkSync.disconnect();
    setArkConnected(false);
    setArkMessage("Отключено от Ark.");
  };

  return (
    <div className="h-full min-h-0 w-full overflow-auto bg-black/55 p-4">
      <div className="mx-auto flex w-full max-w-lg flex-col gap-4 py-6">
        <section className="bg-(--background) border-(--border) w-full rounded-xl border p-5">
          <h2 className="mb-3 inline-flex items-center gap-2 text-base font-semibold">
            <Database size={16} />
            Ark Server
          </h2>
          <p className="text-(--muted-foreground) mb-4 text-sm">
            Синхронизация между устройствами через Ark.
          </p>

          {paired ? (
            <>
              <div className="mb-4 flex items-center gap-2 text-sm">
                <div
                  className={`h-2 w-2 rounded-full ${arkConnected ? "bg-emerald-500" : "bg-rose-500"}`}
                />
                <span className="text-(--muted-foreground)">
                  {arkConnected ? "Подключено" : "Отключено"}
                </span>
              </div>

              <label className="mb-2 block text-sm font-medium">Сервер</label>
              <input
                value={getArkUrl()}
                readOnly
                className="border-(--border) bg-(--secondary) text-(--muted-foreground) mb-4 w-full rounded-md border px-3 py-2 text-sm font-mono"
              />

              <div className="flex items-center gap-2">
                {arkConnected ? (
                  <button
                    type="button"
                    onClick={handleDisconnectArk}
                    className="bg-(--secondary) text-(--secondary-foreground) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
                  >
                    Отключиться
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={handleReconnectArk}
                    className="bg-(--primary) text-(--primary-foreground) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
                  >
                    Подключиться
                  </button>
                )}
                <button
                  type="button"
                  onClick={handleUnpair}
                  className="text-(--destructive) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
                >
                  <Unlink size={16} />
                  Отвязать
                </button>
              </div>
            </>
          ) : (
            <>
              <label
                className="mb-2 block text-sm font-medium"
                htmlFor="connection-code"
              >
                Код подключения
              </label>
              <input
                id="connection-code"
                value={connectionCode}
                onChange={(event) => setConnectionCode(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") handleConnect();
                }}
                className="border-(--border) bg-(--secondary) mb-4 w-full rounded-md border px-3 py-2 text-sm font-mono"
                placeholder="ark://192.168.1.5:8000?key=..."
                autoComplete="off"
                autoCapitalize="off"
              />

              <button
                type="button"
                onClick={handleConnect}
                className="bg-(--primary) text-(--primary-foreground) inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm"
              >
                <Link size={16} />
                Подключить
              </button>
            </>
          )}

          {arkMessage ? (
            <p className="text-(--muted-foreground) mt-3 text-sm">
              {arkMessage}
            </p>
          ) : null}
        </section>

        <section className="bg-(--background) border-(--border) w-full rounded-xl border p-5">
          <h2 className="mb-3 inline-flex items-center gap-2 text-base font-semibold">
            <SunMoon size={16} />
            Theme
          </h2>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => setTheme("light")}
              className={`rounded-md px-3 py-2 text-sm ${
                theme === "light"
                  ? "bg-(--primary) text-(--primary-foreground)"
                  : "bg-(--secondary)"
              }`}
            >
              Light
            </button>
            <button
              type="button"
              onClick={() => setTheme("dark")}
              className={`rounded-md px-3 py-2 text-sm ${
                theme === "dark"
                  ? "bg-(--primary) text-(--primary-foreground)"
                  : "bg-(--secondary)"
              }`}
            >
              Dark
            </button>
            <button
              type="button"
              onClick={() => setTheme("system")}
              className={`rounded-md px-3 py-2 text-sm ${
                theme === "system"
                  ? "bg-(--primary) text-(--primary-foreground)"
                  : "bg-(--secondary)"
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
