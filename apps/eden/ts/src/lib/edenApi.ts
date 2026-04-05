export const edenApi = {
  listEntries: () => window.api.listEntries(),

  loadEntry: (id: string) => window.api.loadEntry(id),

  saveEntry: (entry: Entry) => window.api.saveEntry(entry),

  listFolders: () => window.api.listFolders(),

  createFolder: (id: string, name: string, parentId: string | null = null) =>
    window.api.createFolder(id, name, parentId),

  moveEntryToFolder: (entryId: string, folderId: string | null) =>
    window.api.moveEntryToFolder(entryId, folderId),

  moveFolderToFolder: (folderId: string, parentId: string | null) =>
    window.api.moveFolderToFolder(folderId, parentId),

  deleteEntry: (entryId: string) => window.api.deleteEntry(entryId),

  deleteFolder: (folderId: string) => window.api.deleteFolder(folderId),

  listNoteTypes: () => window.api.listNoteTypes(),

  saveNoteType: (noteType: NoteType) => window.api.saveNoteType(noteType),

  deleteNoteType: (noteTypeId: string) => window.api.deleteNoteType(noteTypeId),

  searchEntries: (query: string) => window.api.searchEntries(query),

  getVaultPath: () => window.api.getVaultPath(),

  setVaultPath: (path: string) => window.api.setVaultPath(path),

  selectFolder: () => window.api.selectFolder(),
};
