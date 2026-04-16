import { contextBridge, ipcRenderer } from "electron";
import { ARRANCADOR_BRIDGE_KEY, createArrancadorBridge } from "./shared/ipc";

const bridge = createArrancadorBridge((channel, payload) =>
  ipcRenderer.invoke(channel, payload),
);

contextBridge.exposeInMainWorld(ARRANCADOR_BRIDGE_KEY, bridge);
