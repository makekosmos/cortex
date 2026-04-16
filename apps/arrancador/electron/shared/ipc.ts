import { ipcRenderer } from "electron";
import type {
  ArrancadorBridge,
  ArrancadorEventCallback,
  ArrancadorEventName,
} from "../../src/types/ipc";

export { ARRANCADOR_BRIDGE_KEY } from "../../src/types/ipc";

export const createArrancadorBridge = (
  invoke: (channel: string, payload?: unknown) => Promise<unknown>,
): ArrancadorBridge => ({
  invoke: ((channel, ...args) =>
    (args.length === 0
      ? invoke(channel)
      : invoke(channel, args[0]))) as ArrancadorBridge["invoke"],
  on: ((event, callback) => {
    const listener = (_electronEvent: unknown, payload: unknown) => {
      (callback as (value: unknown) => void)(payload);
    };

    ipcRenderer.on(event, listener);
    return () => {
      ipcRenderer.removeListener(event, listener);
    };
  }) as <E extends ArrancadorEventName>(
    event: E,
    callback: ArrancadorEventCallback<E>,
  ) => () => void,
});

export const electronRendererInvoke = (
  channel: string,
  payload?: unknown,
) => ipcRenderer.invoke(channel, payload);
