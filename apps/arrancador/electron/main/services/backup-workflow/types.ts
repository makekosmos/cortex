import type { DbLike, DbValue } from "../../helpers/shared";
import type {
  BackupInfo,
  BackupRecord,
  CreateBackupInput,
  DeleteBackupInput,
  FindSavePathInput,
  GameManifest,
  RestoreBackupInput,
  RestoreCheck,
  SaveDiscovery,
  SavePathLookup,
} from "../backup/types";
import type { BackupListItem, GameBackupState } from "../backup-ipc-support";
import type { AppSettings } from "../contracts";

export type BackupRecordRow = {
  id: string;
  game_id: string;
  backup_path: string;
  backup_size: number;
  created_at: string;
  is_auto: number | boolean;
  notes: string | null;
};

export type BackupProgressEvent = {
  game_id: string;
  stage: string;
  message: string;
  done: number;
  total: number;
};

export interface BackupWorkflowSettingsPort {
  getAllSettings(): Promise<AppSettings>;
}

export interface BackupWorkflowAchievementsPort {
  recordAchievementEvent(
    eventTrigger: string,
    eventContext?: string | null,
  ): Promise<unknown>;
}

export interface BackupWorkflowInternals {
  execute(
    db: DbLike,
    sql: string,
    params?: readonly DbValue[],
  ): Promise<number>;
  queryAll<T>(
    db: DbLike,
    sql: string,
    params?: readonly DbValue[],
  ): Promise<T[]>;
  queryOne<T>(
    db: DbLike,
    sql: string,
    params?: readonly DbValue[],
  ): Promise<T | undefined>;
  runInTransaction<T>(
    db: DbLike,
    fn: (tx: DbLike) => T | Promise<T>,
  ): Promise<T>;
  getSetting(db: DbLike, key: string): Promise<string | null>;
  setSetting(db: DbLike, key: string, value: string): Promise<void>;
  getBackupRoot(db: DbLike, userDataPath: string): Promise<string>;
  getGameBackupState(db: DbLike, id: string): Promise<GameBackupState>;
  getLatestBackup(
    db: DbLike,
    gameId: string,
  ): Promise<BackupListItem | null>;
  listGameBackups(db: DbLike, gameId: string): Promise<BackupListItem[]>;
  loadManifestFromCache(userDataPath: string): Promise<GameManifest | null>;
  reconcileBackupRows(
    db: DbLike,
    gameId: string,
    maxBackups: number,
  ): Promise<void>;
  releasedYear(released: string | null): string | null;
  resolveGameOverridePath(
    db: DbLike,
    gameId: string,
    rawSavePath: string | null,
  ): Promise<string | null>;
  findSavePath(input: FindSavePathInput): Promise<SavePathLookup>;
  discoverBackupInfo(
    gameName: string,
    manifest?: GameManifest | null,
    overridePath?: string | null,
  ): Promise<BackupInfo | null>;
  createBackupArtifact(input: CreateBackupInput): Promise<BackupRecord>;
  restoreBackupArtifact(input: RestoreBackupInput): Promise<void>;
  deleteBackupArtifact(input: DeleteBackupInput): Promise<void>;
  findGameSaveArtifacts(
    gameName: string,
    manifest?: GameManifest | null,
    overridePath?: string | null,
  ): Promise<SaveDiscovery | null>;
  evaluateBackupNeeded(input: {
    currentSave: SaveDiscovery | null;
    lastBackup: BackupRecord | null;
  }): Promise<boolean>;
  evaluateRestoreNeeded(input: {
    currentSave: SaveDiscovery | null;
    lastBackup: BackupRecord | null;
  }): Promise<RestoreCheck>;
}

export interface BackupWorkflowDeps {
  db: DbLike;
  userDataPath: string;
  settings: BackupWorkflowSettingsPort;
  achievements: BackupWorkflowAchievementsPort;
  emitRendererEvent: (channel: string, payload: unknown) => void;
  internals?: Partial<BackupWorkflowInternals>;
}

export interface FindGameSavePathsPayload {
  gameName: string;
  gameId?: string;
}

export interface CreateBackupPayload {
  gameId: string;
  gameName: string;
  isAuto: boolean;
  notes?: string;
}

export interface BackupCheckPayload {
  gameId: string;
  gameName: string;
}

export interface BackupWorkflowContext {
  deps: BackupWorkflowDeps;
  io: BackupWorkflowInternals;
}
