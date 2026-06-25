export async function listFolders(): Promise<Folder[]> {
  return [];
}

export async function createFolder(
  _id: string,
  _name: string,
  _parentId: string | null = null,
): Promise<CreateFolderResult> {
  return {
    ok: false,
    reason: "duplicate_folder_name",
    name: "Папки недоступны в extension'е (Phase 6.0)",
  };
}

export async function moveEntryToFolder(
  _entryId: string,
  _folderId: string | null,
): Promise<boolean> {
  return false;
}

export async function moveFolderToFolder(
  _folderId: string,
  _parentId: string | null,
): Promise<MoveFolderResult> {
  return {
    ok: false,
    reason: "folder_not_found",
    message: "Папки недоступны в extension'е (Phase 6.0)",
  };
}

export async function deleteFolder(_folderId: string): Promise<DeleteFolderResult> {
  return {
    ok: false,
    reason: "folder_not_found",
    message: "Папки недоступны в extension'е (Phase 6.0)",
  };
}
