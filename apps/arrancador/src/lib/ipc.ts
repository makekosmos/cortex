import type { IpcChannel } from "@/types/ipc";

type AppInvokeFn = <T = unknown>(
  channel: IpcChannel,
  payload?: unknown,
) => Promise<T>;

const getElectronInvoke = () =>
  typeof window === "undefined" ? undefined : window.arrancador?.invoke;

const hasTauriRuntime = () => {
  if (typeof window === "undefined") {
    return false;
  }

  const runtime = window as typeof window & {
    __TAURI_INTERNALS__?: unknown;
    __TAURI__?: unknown;
  };

  return Boolean(runtime.__TAURI_INTERNALS__ || runtime.__TAURI__);
};

let tauriInvokePromise: Promise<
  ((channel: string, payload?: unknown) => Promise<unknown>) | null
> | null = null;

const getTauriInvoke = async () => {
  if (!hasTauriRuntime()) {
    return null;
  }

  if (!tauriInvokePromise) {
    tauriInvokePromise = import("@tauri-apps/api/core")
      .then((module) => module.invoke as (channel: string, payload?: unknown) => Promise<unknown>)
      .catch(() => null);
  }

  return await tauriInvokePromise;
};

export const invoke: AppInvokeFn = ((
  channel: IpcChannel,
  ...args: [unknown?]
) => {
  const payload = args[0];
  const electronInvoke = getElectronInvoke();

  if (electronInvoke) {
    return (args.length === 0
      ? electronInvoke(channel as never)
      : electronInvoke(channel as never, payload as never)) as Promise<unknown>;
  }

  return getTauriInvoke().then((tauriInvoke) => {
    if (!tauriInvoke) {
      throw new Error(`IPC bridge is unavailable for channel "${channel}"`);
    }

    return (args.length === 0
      ? tauriInvoke(channel)
      : tauriInvoke(channel, payload)) as Promise<unknown>;
  });
}) as AppInvokeFn;
