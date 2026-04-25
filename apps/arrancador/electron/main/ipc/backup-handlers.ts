import { ipcMain } from "electron";
import { createBackupWorkflow } from "../services/backup-workflow";
import type { WithRuntime } from "./types";

export interface BackupIpcDeps {
  withRuntime: WithRuntime;
  getUserDataPath: () => string;
  emitRendererEvent: (channel: string, payload: unknown) => void;
}

export function registerBackupIpcHandlers(deps: BackupIpcDeps) {
  const { withRuntime } = deps;

  const withBackupWorkflow = <TPayload, TResult>(
    handler: (
      workflow: ReturnType<typeof createBackupWorkflow>,
      payload: TPayload,
    ) => Promise<TResult> | TResult,
  ) =>
    withRuntime(async ({ db, services }, payload: TPayload) => {
      const workflow = createBackupWorkflow({
        db,
        userDataPath: deps.getUserDataPath(),
        settings: services.settings,
        achievements: services.achievements,
        emitRendererEvent: deps.emitRendererEvent,
      });

      return await handler(workflow, payload);
    });

  ipcMain.handle(
    "check_ludusavi_installed",
    withBackupWorkflow(async (workflow) => await workflow.checkLudusaviInstalled()),
  );
  ipcMain.handle(
    "get_ludusavi_executable_path",
    withBackupWorkflow(async (workflow) => await workflow.getLudusaviExecutablePath()),
  );
  ipcMain.handle(
    "set_ludusavi_path",
    withBackupWorkflow(
      async (workflow, payload: { path: string }) =>
        await workflow.setLudusaviPath(payload.path),
    ),
  );
  ipcMain.handle(
    "set_backup_directory",
    withBackupWorkflow(
      async (workflow, payload: { path: string }) =>
        await workflow.setBackupDirectory(payload.path),
    ),
  );
  ipcMain.handle(
    "get_backup_directory_setting",
    withBackupWorkflow(async (workflow) => await workflow.getBackupDirectorySetting()),
  );
  ipcMain.handle(
    "refresh_sqoba_manifest",
    withBackupWorkflow(async (workflow) => await workflow.refreshSqobaManifest()),
  );
  ipcMain.handle(
    "find_game_save_paths",
    withBackupWorkflow(
      async (
        workflow,
        payload: { gameName: string; gameId?: string },
      ) => await workflow.findGameSavePaths(payload),
    ),
  );
  ipcMain.handle(
    "find_game_saves",
    withBackupWorkflow(
      async (
        workflow,
        payload: { gameName: string; gameId?: string },
      ) => await workflow.findGameSaves(payload),
    ),
  );
  ipcMain.handle(
    "create_backup",
    withBackupWorkflow(
      async (
        workflow,
        payload: {
          gameId: string;
          gameName: string;
          isAuto: boolean;
          notes?: string;
        },
      ) => await workflow.createBackup(payload),
    ),
  );
  ipcMain.handle(
    "get_game_backups",
    withBackupWorkflow(
      async (workflow, payload: { gameId: string }) =>
        await workflow.getGameBackups(payload.gameId),
    ),
  );
  ipcMain.handle(
    "restore_backup",
    withBackupWorkflow(
      async (workflow, payload: { backupId: string }) =>
        await workflow.restoreBackup(payload.backupId),
    ),
  );
  ipcMain.handle(
    "delete_backup",
    withBackupWorkflow(
      async (workflow, payload: { backupId: string }) =>
        await workflow.deleteBackup(payload.backupId),
    ),
  );
  ipcMain.handle(
    "should_backup_before_launch",
    withBackupWorkflow(
      async (workflow, payload: { gameId: string }) =>
        await workflow.shouldBackupBeforeLaunch(payload.gameId),
    ),
  );
  ipcMain.handle(
    "check_backup_needed",
    withBackupWorkflow(
      async (
        workflow,
        payload: { gameId: string; gameName: string },
      ) => await workflow.checkBackupNeeded(payload),
    ),
  );
  ipcMain.handle(
    "check_restore_needed",
    withBackupWorkflow(
      async (
        workflow,
        payload: { gameId: string; gameName: string },
      ) => await workflow.checkRestoreNeeded(payload),
    ),
  );
  ipcMain.handle(
    "get_backup_settings",
    withBackupWorkflow(async (workflow) => await workflow.getBackupSettings()),
  );
  ipcMain.handle(
    "update_backup_settings",
    withBackupWorkflow(
      async (workflow, payload: { settings: Record<string, string> }) =>
        await workflow.updateBackupSettings(payload.settings),
    ),
  );
}
