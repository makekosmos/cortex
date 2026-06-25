import "./diagnostics";

import { APP_ICON_PROTOCOL } from "./app-icon-protocol";
import { LOCAL_IMAGE_PROTOCOL } from "./local-image-protocol";
import { stopClipboardHistory } from "./clipboard-history";
import { findKextInArgv, openInstallExtensionWindow } from "./install-extension-window";
import { safeHandle } from "./ipc-safe";
import { keplerLog } from "./logging";
import { resolveWindowMaterial, type KosmosWindowMaterial } from "./window-effects";

export {
  APP_ICON_PROTOCOL,
  LOCAL_IMAGE_PROTOCOL,
  findKextInArgv,
  keplerLog,
  openInstallExtensionWindow,
  resolveWindowMaterial,
  safeHandle,
  stopClipboardHistory,
  type KosmosWindowMaterial,
};
