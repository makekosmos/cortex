import type { IpcChannel } from "@/types/ipc";

type AppInvokeFn = <T = unknown>(
  channel: IpcChannel,
  payload?: unknown,
) => Promise<T>;

const getElectronCommands = () =>
  typeof window === "undefined" ? undefined : window.arrancador?.commands;

export const invoke: AppInvokeFn = ((
  channel: IpcChannel,
  ...args: [unknown?]
) => {
  const payload = args[0];
  const commands = getElectronCommands();
  const command = commands?.[channel] as
    | ((payload?: unknown) => Promise<unknown>)
    | undefined;

  if (command) {
    return (args.length === 0
      ? command()
      : command(payload)) as Promise<unknown>;
  }

  return Promise.reject(
    new Error(`IPC bridge is unavailable for channel "${channel}"`),
  );
}) as AppInvokeFn;
