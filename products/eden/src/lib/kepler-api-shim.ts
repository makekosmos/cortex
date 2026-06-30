// kepler-api-shim — эмуляция `window.api` поверх `window.kepler.ark.request`.
//
// Eden как Kepler extension не имеет собственного Electron main процесса:
// все ARK операции идут через main proxy в kepler-shell. Этот модуль
// устанавливает `window.api` с такой же сигнатурой как в standalone Eden
// (см. `apps/eden/ts/main/preload.ts`), чтобы существующие store / components
// продолжали работать без массового rewrite call-sites.
//
// Phase 6.0 (2026-05-17): note CRUD + folders + note types + search через ARK.
// Phase 6.0.A (2026-05-17): code tools / vault picker / export — удалены.
// Trash работает поверх ARK soft-delete (deletedAt != null).

import {
  createTask,
  ensureTaskObjectTypeRegistered,
  getTask,
  patchTask,
  softDeleteTask,
  subscribeObjectChanges,
} from "./kepler-task-sync";
import {
  close,
  getEdenVisibleObjectTypeIds,
  getPlatform,
  getRecentVaultPaths,
  getSidebarConfig,
  getVaultPath,
  maximize,
  minimize,
  openMarkdownFile,
  openMarkdownVault,
  readVisibleObjectTypeIds,
  saveMarkdownFile,
  saveMarkdownVault,
  selectFolder,
  setEdenVisibleObjectTypeIds,
  setVaultPath,
  updateSidebarConfig,
  zoomGet,
  zoomSet,
} from "./kepler-ui-runtime";
import { onCommand } from "./kepler-command-bus";
import { createEntryApi } from "./kepler-entry-api";
import {
  createFolder,
  deleteFolder,
  listFolders,
  moveEntryToFolder,
  moveFolderToFolder,
} from "./kepler-folder-stubs";
import { createTrashStorageApi } from "./kepler-trash-storage";
export {
  createFolder,
  deleteFolder,
  getVaultPath,
  listFolders,
  moveEntryToFolder,
  moveFolderToFolder,
  selectFolder,
  setVaultPath,
};
export {
  createTask,
  ensureTaskObjectTypeRegistered,
  getTask,
  patchTask,
  softDeleteTask,
  subscribeObjectChanges,
} from "./kepler-task-sync";
export { dispatchEdenCommand } from "./kepler-command-bus";

interface KeplerArkBridge {
  request: <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;
  subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
}

interface KeplerNamespace {
  ark: KeplerArkBridge;
}

function keplerBridge(): KeplerArkBridge {
  const k = (window as unknown as { kepler?: KeplerNamespace }).kepler;
  if (!k?.ark) {
    throw new Error("Eden extension: window.kepler.ark недоступен — preload не подключён");
  }
  return k.ark;
}

function ark<T = unknown>(operation: string, params?: Record<string, unknown>): Promise<T> {
  return keplerBridge().request<T>(operation, params);
}

export const {
  loadListableEntry,
  listAllEntries,
  listEntries,
  loadEntry,
  saveEntry,
  deleteEntry,
  listNoteTypes,
  ensureCollectionObjects,
  saveNoteType,
  deleteNoteType,
  searchEntries,
} = createEntryApi(ark, readVisibleObjectTypeIds);

const {
  listTrashEntries,
  restoreEntry,
  permanentDeleteEntry,
  purgeExpiredTrash,
  getVaultStorageInfo,
  getDiskFreeSpace,
} = createTrashStorageApi(ark);

// ---------------------------------------------------------------------------
// Install.
// ---------------------------------------------------------------------------

export function installKeplerApiShim(): void {
  (window as unknown as { api: Window["api"] }).api = {
    saveEntry,
    loadEntry,
    listEntries,
    listAllEntries,
    getVaultPath,
    getRecentVaultPaths,
    selectFolder,
    setVaultPath,
    exportMarkdownVault: saveMarkdownVault,
    openMarkdownFile,
    saveMarkdownFile,
    openMarkdownVault,
    searchEntries,
    createFolder,
    listFolders,
    listNoteTypes,
    ensureCollectionObjects,
    saveNoteType,
    deleteNoteType,
    moveEntryToFolder,
    moveFolderToFolder,
    deleteEntry,
    deleteFolder,
    getSidebarConfig,
    updateSidebarConfig,
    getEdenVisibleObjectTypeIds,
    setEdenVisibleObjectTypeIds,
    getPlatform,
    listTrashEntries,
    restoreEntry,
    permanentDeleteEntry,
    purgeExpiredTrash,
    getVaultStorageInfo,
    getDiskFreeSpace,
    zoomGet,
    zoomSet,
    minimize,
    maximize,
    close,
    onCommand,
  };
}
