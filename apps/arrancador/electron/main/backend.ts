import fs from "node:fs";
import path from "node:path";

import { app, BrowserWindow, type IpcMainInvokeEvent } from "electron";
import {
  getArkDbPathForSelectedSpace,
  getKosmosDataDir,
  readSharedSelectedSpace,
} from "@kosmos/ark";
import { syncArkRuntimeBinding } from "./ark-runtime";
import { openGameDatabase, openSqliteDatabase } from "./db";
import type { DbLike } from "./helpers/shared";
import { registerAppIpcHandlers } from "./ipc/app-handlers";
import { registerBackupIpcHandlers } from "./ipc/backup-handlers";
import { registerGameIpcHandlers } from "./ipc/game-handlers";
import { registerShellScanIpcHandlers } from "./ipc/shell-scan-handlers";
import { createRuntimeServices } from "./runtime-services";
import { backfillLegacyUsageToArk } from "./services/ark-usage-backfill";
import type { createScanCancellation } from "./services/scan";

export type RuntimeServices = ReturnType<typeof createRuntimeServices>;

export interface RuntimeState {
  db: DbLike;
  services: RuntimeServices;
  currentScan: ReturnType<typeof createScanCancellation> | null;
  arkDbPath: string | null;
}

let runtimeState: RuntimeState | null = null;
let ipcRegistered = false;
let startupArkSyncPromise: Promise<void> | null = null;

function getDbPath() {
  return path.join(app.getPath("userData"), "arrancador.db");
}

function getArkDbPath() {
  const override = process.env.ARK_DB_PATH?.trim();
  if (override) {
    return override;
  }

  return getArkDbPathForSelectedSpace(
    app.getPath("appData"),
    readSharedSelectedSpace(app.getPath("appData")),
  );
}

function getRootArkDbPath() {
  const override = process.env.ARK_DB_PATH?.trim();
  if (override) {
    return override;
  }

  return path.join(getKosmosDataDir(app.getPath("appData")), "ark.db");
}

function getArkConnectionInfo() {
  const sharedSelection = readSharedSelectedSpace(app.getPath("appData"));
  const arkDbPath = getArkDbPath();

  return {
    ark_db_path: arkDbPath,
    ark_db_exists: fs.existsSync(arkDbPath),
    ark_db_directory: path.dirname(arkDbPath),
    uses_shared_selection: sharedSelection !== null,
    space_code: sharedSelection?.spaceCode ?? null,
    space_id: sharedSelection?.spaceId ?? null,
    selection_source: sharedSelection?.source ?? null,
    vault_path: sharedSelection?.vaultPath ?? null,
  };
}

function emitRendererEvent(channel: string, payload: unknown) {
  for (const window of BrowserWindow.getAllWindows()) {
    window.webContents.send(channel, payload);
  }
}

function getRuntimeState(): RuntimeState {
  const state = runtimeState;
  if (!state) {
    throw new Error("App runtime is not initialized");
  }

  const appDataPath = app.getPath("appData");
  const next = syncArkRuntimeBinding(
    {
      arkDbPath: state.arkDbPath,
      services: state.services,
    },
    {
      appDataPath,
      selection: readSharedSelectedSpace(appDataPath),
      createServices: (arkDbPath) =>
        createRuntimeServices({
          db: state.db,
          arkDbPath,
          fallbackArkDbPath: getRootArkDbPath(),
        }),
    },
  );

  if (next.changed) {
    runtimeState.arkDbPath = next.arkDbPath;
    runtimeState.services = next.services;
  }

  return state;
}

function registerIpcHandlers() {
  const withRuntime = <TArgs extends unknown[], TResult>(
    handler: (runtime: RuntimeState, ...args: TArgs) => Promise<TResult> | TResult,
  ) => {
    return async (_event: IpcMainInvokeEvent, ...args: TArgs) =>
      await handler(getRuntimeState(), ...args);
  };

  registerGameIpcHandlers({ withRuntime, getArkDbPath });
  registerAppIpcHandlers({ withRuntime, getArkConnectionInfo });
  registerShellScanIpcHandlers({ withRuntime, emitRendererEvent });
  registerBackupIpcHandlers({
    withRuntime,
    getUserDataPath: () => app.getPath("userData"),
    emitRendererEvent,
  });
}

async function createRuntime() {
  const arkDbPath = getArkDbPath();
  const fallbackArkDbPath = getRootArkDbPath();
  const db = await openGameDatabase(
    openSqliteDatabase(getDbPath()),
  );

  await backfillLegacyUsageToArk({
    legacyDb: db,
    arkDbPath,
    fallbackArkDbPath,
  }).catch((error) => {
    console.warn("Legacy usage backfill to Ark failed:", error);
  });

  runtimeState = {
    db,
    services: createRuntimeServices({
      db,
      arkDbPath,
      fallbackArkDbPath,
    }),
    currentScan: null,
    arkDbPath,
  };
}

export function triggerStartupArkSync(): void {
  if (startupArkSyncPromise) {
    return;
  }

  if (!runtimeState) {
    return;
  }

  const runtime = getRuntimeState();

  startupArkSyncPromise = runtime.services.games
    .syncAllGamesToArk()
    .then(() => undefined)
    .catch((error) => {
      console.warn("Startup Ark game sync failed:", error);
    })
    .finally(() => {
      startupArkSyncPromise = null;
    });
}

export async function initializeBackend(): Promise<void> {
  if (!runtimeState) {
    await createRuntime();
  }

  if (!ipcRegistered) {
    registerIpcHandlers();
    ipcRegistered = true;
  }
}
