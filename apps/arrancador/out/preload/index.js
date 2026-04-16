// electron/preload.ts
import { contextBridge, ipcRenderer as ipcRenderer2 } from "electron";

// electron/shared/ipc.ts
import { ipcRenderer } from "electron";

// src/types/ipc.ts
var ARRANCADOR_BRIDGE_KEY = "arrancador";

// electron/shared/ipc.ts
var createArrancadorBridge = (invoke) => ({
  invoke: (channel, ...args) => args.length === 0 ? invoke(channel) : invoke(channel, args[0]),
  on: (event, callback) => {
    const listener = (_electronEvent, payload) => {
      callback(payload);
    };
    ipcRenderer.on(event, listener);
    return () => {
      ipcRenderer.removeListener(event, listener);
    };
  }
});

// electron/preload.ts
var bridge = createArrancadorBridge((channel, payload) => ipcRenderer2.invoke(channel, payload));
contextBridge.exposeInMainWorld(ARRANCADOR_BRIDGE_KEY, bridge);
