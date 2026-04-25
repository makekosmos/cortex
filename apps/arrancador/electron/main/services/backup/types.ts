export type BackupArtifactKind = "directory" | "zip";

export type BackupCompressionPreset = "fastest" | "optimal";

export interface GameManifestEntry {
  files?: Record<string, string[]> | null;
  registry?: string[] | null;
}

export interface GameManifest {
  games: Record<string, GameManifestEntry>;
}

export interface BackupFileEntry {
  backupPath: string;
  originalPath: string;
  size: number;
  mtime?: number | null;
}

export interface BackupArchiveManifest {
  version: number;
  files: BackupFileEntry[];
}

export interface SaveRoot {
  label: string;
  path: string;
}

export interface SaveFile {
  path: string;
  rootLabel: string;
  relativePath: string;
  size: number;
}

export interface SaveDiscovery {
  roots: SaveRoot[];
  files: SaveFile[];
  totalSize: number;
}

export interface BackupRecord {
  id?: string;
  gameId?: string;
  backupPath: string;
  backupSize: number;
  createdAt: string;
  isAuto?: boolean;
  notes?: string | null;
}

export interface BackupArtifactSummary extends BackupRecord {
  kind: BackupArtifactKind;
  saveRoot: string | null;
}

export interface BackupInfo {
  gameName: string;
  savePath: string | null;
  registryPath: string | null;
  totalSize: number;
  files: string[];
}

export interface SavePathLookup {
  savePath: string | null;
  candidates: string[];
}

export interface RestoreCheck {
  shouldRestore: boolean;
  backupId: string | null;
  currentSize: number;
  backupSize: number;
}

export interface BackupProgress {
  stage: "scan" | "copy" | "restore" | "done";
  current: string;
  done: number;
  total: number;
}

export type ProgressListener = (progress: BackupProgress) => void | Promise<void>;

export interface FindSavePathInput {
  gameName: string;
  gameId?: string | null;
  overridePath?: string | null;
  manifest?: GameManifest | null;
}

export interface CreateBackupInput {
  gameId: string;
  gameName: string;
  backupRoot: string;
  mode?: BackupArtifactKind;
  compressionLevel?: number;
  skipCompressionOnce?: boolean;
  manifest?: GameManifest | null;
  overridePath?: string | null;
  isAuto?: boolean;
  notes?: string | null;
  gameYear?: string | null;
  maxBackupsPerGame?: number | null;
  onProgress?: ProgressListener | null;
}

export interface RestoreBackupInput {
  backupPath: string;
  allowedRestoreRoots: string[];
  onProgress?: ProgressListener | null;
}

export interface ListBackupsInput {
  backupRoot: string;
  gameName: string;
  gameYear?: string | null;
}

export interface DeleteBackupInput {
  backupPath: string;
}

export interface CheckBackupNeededInput {
  currentSave: SaveDiscovery | null;
  lastBackup: BackupRecord | null;
}

export interface CheckRestoreNeededInput {
  currentSave: SaveDiscovery | null;
  lastBackup: BackupRecord | null;
}
