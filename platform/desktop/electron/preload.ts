// Preload script: bridge между renderer'ом и main process.
//
// Renderer не имеет прямого доступа к Node/Electron API — только через
// exposed `window.kepler` contract из shared/ipc-types.ts.

import { contextBridge } from "electron";
import { createKeplerPreloadApi, installKeplerPlatformMarker } from "./preload-bridge";

installKeplerPlatformMarker();
const api = createKeplerPreloadApi();

contextBridge.exposeInMainWorld("kepler", api);
