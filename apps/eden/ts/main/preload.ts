import { ipcRenderer, contextBridge } from "electron";

import type {
  CreateFolderResult,
  DeleteEntryResult,
  DeleteFolderResult,
  Entry,
  ExportMarkdownVaultResult,
  MoveFolderResult,
  SaveEntryResult,
  SaveNoteTypeResult,
} from "./store";

import type { NoteType } from "@/lib/typedNotes";

import type {
  CodeFormatResult,
  CodeLintResult,
  CodeToolsSettings,
} from "./codeTools";

import type { HevyLoginResponse } from "./hevy"; // eslint-disable-line

// --------- Expose some API to the Renderer process ---------

// NOTE: Only expose specific, validated API methods via window.api below.

// A raw ipcRenderer bridge (on/off/send/invoke) is intentionally NOT exposed

// because it allows the renderer to call ANY IPC channel, bypassing the

// whitelisted API surface. If XSS occurs, an attacker could invoke arbitrary

// IPC handlers (e.g., delete entries, exec commands, access filesystem).

contextBridge.exposeInMainWorld("api", {
  saveEntry: (entry: Entry): Promise<SaveEntryResult> =>
    ipcRenderer.invoke("save-entry", entry),

  loadEntry: (id: string) => ipcRenderer.invoke("load-entry", id),

  listEntries: () => ipcRenderer.invoke("list-entries"),

  getVaultPath: () => ipcRenderer.invoke("get-vault-path"),

  getRecentVaultPaths: (): Promise<string[]> =>
    ipcRenderer.invoke("get-recent-vault-paths"),

  selectFolder: () => ipcRenderer.invoke("select-folder"),

  setVaultPath: (path: string) => ipcRenderer.invoke("set-vault-path", path),

  exportMarkdownVault: (): Promise<ExportMarkdownVaultResult | null> =>
    ipcRenderer.invoke("export-markdown-vault"),

  searchEntries: (query: string) => ipcRenderer.invoke("search-entries", query),

  createFolder: (
    id: string,

    name: string,

    parentId: string | null = null,
  ): Promise<CreateFolderResult> =>
    ipcRenderer.invoke("create-folder", id, name, parentId),

  listFolders: () => ipcRenderer.invoke("list-folders"),

  listNoteTypes: (): Promise<NoteType[]> =>
    ipcRenderer.invoke("list-note-types"),

  saveNoteType: (noteType: NoteType): Promise<SaveNoteTypeResult> =>
    ipcRenderer.invoke("save-note-type", noteType),

  deleteNoteType: (noteTypeId: string): Promise<boolean> =>
    ipcRenderer.invoke("delete-note-type", noteTypeId),

  moveEntryToFolder: (entryId: string, folderId: string | null) =>
    ipcRenderer.invoke("move-entry-to-folder", entryId, folderId),

  moveFolderToFolder: (
    folderId: string,
    parentId: string | null,
  ): Promise<MoveFolderResult> =>
    ipcRenderer.invoke("move-folder-to-folder", folderId, parentId),

  deleteEntry: (entryId: string): Promise<DeleteEntryResult> =>
    ipcRenderer.invoke("delete-entry", entryId),

  deleteFolder: (folderId: string): Promise<DeleteFolderResult> =>
    ipcRenderer.invoke("delete-folder", folderId),

  getCodeToolsSettings: (): Promise<CodeToolsSettings> =>
    ipcRenderer.invoke("get-code-tools-settings"),

  updateCodeToolsSettings: (
    settings: Partial<CodeToolsSettings>,
  ): Promise<CodeToolsSettings> =>
    ipcRenderer.invoke("update-code-tools-settings", settings),

  lintCodeBlock: (language: string, code: string): Promise<CodeLintResult> =>
    ipcRenderer.invoke("lint-code-block", language, code),

  formatCodeBlock: (
    language: string,
    code: string,
  ): Promise<CodeFormatResult> =>
    ipcRenderer.invoke("format-code-block", language, code),

  getSidebarConfig: (): Promise<{
    widget: { width: number; collapsed: boolean };
  }> => ipcRenderer.invoke("get-sidebar-config"),

  updateSidebarConfig: (config: {
    widget?: { width?: number; collapsed?: boolean };
  }): Promise<{
    widget: { width: number; collapsed: boolean };
  }> => ipcRenderer.invoke("update-sidebar-config", config),

  listTrashEntries: (): Promise<Entry[]> =>
    ipcRenderer.invoke("list-trash-entries"),

  restoreEntry: (entryId: string) =>
    ipcRenderer.invoke("restore-entry", entryId),

  permanentDeleteEntry: (entryId: string) =>
    ipcRenderer.invoke("permanent-delete-entry", entryId),

  purgeExpiredTrash: () => ipcRenderer.invoke("purge-expired-trash"),

  getVaultStorageInfo: () => ipcRenderer.invoke("get-vault-storage-info"),

  getDiskFreeSpace: (): Promise<number> =>
    ipcRenderer.invoke("get-disk-free-space"),

  getPlatform: (): Promise<NodeJS.Platform> =>
    ipcRenderer.invoke("get-platform"),

  zoomGet: (): Promise<number> => ipcRenderer.invoke("zoom-get"),

  zoomSet: (factor: number): Promise<number> =>
    ipcRenderer.invoke("zoom-set", factor),

  hevyLogin: (): Promise<HevyLoginResponse> => ipcRenderer.invoke("hevy-login"),

  hevyLogout: (): Promise<{ ok: boolean }> => ipcRenderer.invoke("hevy-logout"),

  hevyGetAuthStatus: (): Promise<{
    loggedIn: boolean;
    username: string | null;
  }> => ipcRenderer.invoke("hevy-get-auth-status"),

  hevyGetAccount: () => ipcRenderer.invoke("hevy-get-account"),

  hevyGetWorkoutCount: () => ipcRenderer.invoke("hevy-get-workout-count"),

  hevyFetchWorkouts: (startIndex?: number) =>
    ipcRenderer.invoke("hevy-fetch-workouts", startIndex),

  hevyFetchAllWorkouts: () => ipcRenderer.invoke("hevy-fetch-all-workouts"),

  hevySyncWorkouts: () => ipcRenderer.invoke("hevy-sync-workouts"),

  minimize: () => ipcRenderer.send("window-min"),

  maximize: () => ipcRenderer.send("window-max"),

  close: () => ipcRenderer.send("window-close"),
});
