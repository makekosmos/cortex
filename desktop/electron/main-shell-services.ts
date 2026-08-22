import "./diagnostics";

import { APP_ICON_PROTOCOL } from "./app-icon-protocol";
import { LOCAL_IMAGE_PROTOCOL } from "../../shared/electron/local-image-protocol";
import { safeHandle } from "./ipc-safe";
import { keplerLog } from "./logging";
import { resolveWindowMaterial, type KosmosWindowMaterial } from "./window-effects";

export {
  APP_ICON_PROTOCOL,
  LOCAL_IMAGE_PROTOCOL,
  keplerLog,
  resolveWindowMaterial,
  safeHandle,
  type KosmosWindowMaterial,
};
