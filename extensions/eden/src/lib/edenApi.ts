// edenApi — публичный фасад для note CRUD / folders / search / typed-notes.
//
// В extension'е (Phase 6.0) роутится через `kepler-api-shim`, который
// внутри вызывает `window.kepler.ark.request(...)`. В standalone Eden
// edenApi обёртывал preload-bridge напрямую — здесь мы заменили этот
// слой shim'ом для совместимости с Kepler-host архитектурой.

import * as shim from "./kepler-api-shim";
import type { NoteType } from "./typedNotes";

export const edenApi = {
  listEntries: () => shim.listEntries(),

  loadEntry: (id: string) => shim.loadEntry(id),

  saveEntry: (entry: Entry) => shim.saveEntry(entry),

  listFolders: () => shim.listFolders(),

  createFolder: (id: string, name: string, parentId: string | null = null) =>
    shim.createFolder(id, name, parentId),

  moveEntryToFolder: (entryId: string, folderId: string | null) =>
    shim.moveEntryToFolder(entryId, folderId),

  moveFolderToFolder: (folderId: string, parentId: string | null) =>
    shim.moveFolderToFolder(folderId, parentId),

  deleteEntry: (entryId: string) => shim.deleteEntry(entryId),

  deleteFolder: (folderId: string) => shim.deleteFolder(folderId),

  listNoteTypes: () => shim.listNoteTypes(),

  saveNoteType: (noteType: NoteType) => shim.saveNoteType(noteType),

  deleteNoteType: (noteTypeId: string) => shim.deleteNoteType(noteTypeId),

  searchEntries: (query: string) => shim.searchEntries(query),

  getVaultPath: () => shim.getVaultPath(),

  setVaultPath: (path: string) => shim.setVaultPath(path),

  selectFolder: () => shim.selectFolder(),

  ensureTaskObjectTypeRegistered: () => shim.ensureTaskObjectTypeRegistered(),

  softDeleteTask: (taskId: string) => shim.softDeleteTask(taskId),

  getTask: (taskId: string) => shim.getTask(taskId),

  patchTask: (taskId: string, patch: { title?: string; isCompleted?: boolean }) =>
    shim.patchTask(taskId, patch),

  createTask: (sourceNoteId: string, title?: string, explicitId?: string) =>
    shim.createTask(sourceNoteId, title, explicitId),

  subscribeObjectChanges: (
    handler: (payload: { event: "object_upserted" | "object_deleted"; id: string; typeId?: string }) => void,
  ) => shim.subscribeObjectChanges(handler),
};
