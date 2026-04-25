import { execute, queryAll, queryOne, runInTransaction } from "../../helpers/db";
import {
  createBackup as createBackupArtifact,
  deleteBackup as deleteBackupArtifact,
  discoverBackupInfo,
  checkBackupNeeded as evaluateBackupNeeded,
  checkRestoreNeeded as evaluateRestoreNeeded,
  findGameSaves as findGameSaveArtifacts,
  findSavePath,
  restoreBackup as restoreBackupArtifact,
} from "../backup";
import {
  getBackupRoot,
  getGameBackupState,
  getLatestBackup,
  listGameBackups,
  loadManifestFromCache,
  reconcileBackupRows,
  releasedYear,
  resolveGameOverridePath,
} from "../backup-ipc-support";
import { getSetting, setSetting } from "../settings-store";
import type { BackupWorkflowInternals } from "./types";

export function createBackupWorkflowInternals(
  overrides: Partial<BackupWorkflowInternals> = {},
): BackupWorkflowInternals {
  return {
    execute,
    queryAll,
    queryOne,
    runInTransaction,
    getSetting,
    setSetting,
    getBackupRoot,
    getGameBackupState,
    getLatestBackup,
    listGameBackups,
    loadManifestFromCache,
    reconcileBackupRows,
    releasedYear,
    resolveGameOverridePath,
    findSavePath,
    discoverBackupInfo,
    createBackupArtifact,
    restoreBackupArtifact,
    deleteBackupArtifact,
    findGameSaveArtifacts,
    evaluateBackupNeeded,
    evaluateRestoreNeeded,
    ...overrides,
  };
}
