import { describe, expect, it, vi } from "vitest";
import type { DbLike } from "../helpers/shared";
import { type BackupWorkflowInternals, createBackupWorkflow } from "./backup-workflow";
import type { AppSettings } from "./contracts";

const db: DbLike = {
  all: () => [],
  get: () => undefined,
  run: () => ({ changes: 0 }),
};

const settings: AppSettings = {
  theme: "dark",
  ludusavi_path: "native",
  backup_directory: "C:\\Backups",
  auto_backup: true,
  backup_before_launch: true,
  backup_compression_enabled: true,
  backup_compression_level: 60,
  backup_skip_compression_once: true,
  max_backups_per_game: 5,
  rawg_api_key: "",
  start_minimized_in_tray: false,
};

function createWorkflow(internals: Partial<BackupWorkflowInternals>) {
  const emitted: Array<{ channel: string; payload: unknown }> = [];
  const recordAchievementEvent = vi.fn(async () => []);

  return {
    emitted,
    recordAchievementEvent,
    workflow: createBackupWorkflow({
      db,
      userDataPath: "C:\\Users\\Kazui\\AppData\\Roaming\\arrancador",
      settings: {
        getAllSettings: async () => settings,
      },
      achievements: {
        recordAchievementEvent,
      },
      emitRendererEvent: (channel, payload) => {
        emitted.push({ channel, payload });
      },
      internals,
    }),
  };
}

describe("createBackupWorkflow", () => {
  it("emits save-path-missing when lookup has no save path", async () => {
    const { workflow, emitted } = createWorkflow({
      loadManifestFromCache: vi.fn(async () => null),
      getGameBackupState: vi.fn(async () => ({
        id: "game-1",
        name: "Arcadia",
        released: null,
        save_path: null,
        backup_enabled: 1,
      })),
      resolveGameOverridePath: vi.fn(async () => null),
      findSavePath: vi.fn(async () => ({
        savePath: null,
        candidates: [],
      })),
    });

    await expect(
      workflow.findGameSavePaths({
        gameId: "game-1",
        gameName: "Arcadia",
      }),
    ).resolves.toEqual({
      save_path: null,
      candidates: [],
    });

    expect(emitted).toEqual([
      {
        channel: "game:save-path-missing",
        payload: {
          game_id: "game-1",
          game_name: "Arcadia",
        },
      },
    ]);
  });

  it("persists backup rows, updates game state, emits progress, and resets one-shot compression", async () => {
    const execute = vi.fn(async () => 1);
    const setSetting = vi.fn(async () => undefined);
    const reconcileBackupRows = vi.fn(async () => undefined);
    const createBackupArtifact = vi.fn(async (input) => {
      await input.onProgress?.({
        stage: "copy",
        current: "C:\\Saves\\slot1.sav",
        done: 1,
        total: 1,
      });
      return {
        id: "backup-1",
        backupPath: "C:\\Backups\\backup-1",
        backupSize: 42,
        createdAt: "2026-04-24T10:00:00.000Z",
        isAuto: true,
        notes: "before launch",
      };
    });
    const { workflow, emitted, recordAchievementEvent } = createWorkflow({
      execute,
      setSetting,
      reconcileBackupRows,
      createBackupArtifact,
      loadManifestFromCache: vi.fn(async () => null),
      getGameBackupState: vi.fn(async () => ({
        id: "game-1",
        name: "Arcadia",
        released: "2022-01-12",
        save_path: "C:\\Saves",
        backup_enabled: 1,
      })),
      resolveGameOverridePath: vi.fn(async () => "C:\\Saves"),
      getBackupRoot: vi.fn(async () => "C:\\Backups"),
      releasedYear: vi.fn(() => "2022"),
    });

    await expect(
      workflow.createBackup({
        gameId: "game-1",
        gameName: "Arcadia",
        isAuto: true,
        notes: "before launch",
      }),
    ).resolves.toMatchObject({
      id: "backup-1",
      game_id: "game-1",
      backup_size: 42,
      is_auto: true,
      notes: "before launch",
    });

    expect(createBackupArtifact).toHaveBeenCalledWith(
      expect.objectContaining({
        gameId: "game-1",
        gameName: "Arcadia",
        backupRoot: "C:\\Backups",
        mode: "directory",
        skipCompressionOnce: true,
        gameYear: "2022",
        maxBackupsPerGame: 5,
      }),
    );
    expect(execute).toHaveBeenCalledWith(
      db,
      expect.stringContaining("INSERT INTO backups"),
      expect.arrayContaining(["backup-1", "game-1"]),
    );
    expect(execute).toHaveBeenCalledWith(
      db,
      expect.stringContaining("UPDATE games"),
      ["2026-04-24T10:00:00.000Z", "game-1"],
    );
    expect(reconcileBackupRows).toHaveBeenCalledWith(db, "game-1", 5);
    expect(setSetting).toHaveBeenCalledWith(
      db,
      "backup_skip_compression_once",
      "false",
    );
    expect(recordAchievementEvent).toHaveBeenCalledWith("backup_restore");
    expect(emitted).toEqual([
      {
        channel: "backup:progress",
        payload: {
          game_id: "game-1",
          stage: "copy",
          message: "C:\\Saves\\slot1.sav",
          done: 1,
          total: 1,
        },
      },
    ]);
  });

  it("rejects restore for missing backup IDs", async () => {
    const { workflow } = createWorkflow({
      queryOne: vi.fn(async () => undefined),
      restoreBackupArtifact: vi.fn(async () => undefined),
    });

    await expect(workflow.restoreBackup("missing")).rejects.toThrow("Backup not found");
  });

  it("restores backups only through current trusted save roots", async () => {
    const restoreBackupArtifact = vi.fn(async () => undefined);
    const { workflow, emitted, recordAchievementEvent } = createWorkflow({
      restoreBackupArtifact,
      loadManifestFromCache: vi.fn(async () => null),
      getGameBackupState: vi.fn(async () => ({
        id: "game-1",
        name: "Arcadia",
        released: null,
        save_path: "C:\\Saves",
        backup_enabled: 1,
      })),
      resolveGameOverridePath: vi.fn(async () => "C:\\Saves"),
      findGameSaveArtifacts: vi.fn(async () => ({
        roots: [{ label: "root-0", path: "C:\\Saves" }],
        files: [
          {
            path: "C:\\Saves\\slot1.sav",
            rootLabel: "root-0",
            relativePath: "slot1.sav",
            size: 9,
          },
        ],
        totalSize: 9,
      })),
      queryOne: vi.fn(async () => ({
        id: "backup-1",
        game_id: "game-1",
        backup_path: "C:\\Backups\\backup-1",
        backup_size: 42,
        created_at: "2026-04-24T10:00:00.000Z",
        is_auto: 0,
        notes: null,
      })),
    });

    await workflow.restoreBackup("backup-1");

    expect(restoreBackupArtifact).toHaveBeenCalledWith({
      backupPath: "C:\\Backups\\backup-1",
      allowedRestoreRoots: ["C:\\Saves"],
      onProgress: expect.any(Function),
    });
    const onProgress = restoreBackupArtifact.mock.calls[0]?.[0].onProgress;
    await onProgress?.({
      stage: "restore",
      current: "C:\\Saves\\slot1.sav",
      done: 1,
      total: 1,
    });
    expect(emitted).toEqual([
      {
        channel: "restore:progress",
        payload: {
          game_id: "game-1",
          stage: "restore",
          message: "C:\\Saves\\slot1.sav",
          done: 1,
          total: 1,
        },
      },
    ]);
    expect(recordAchievementEvent).toHaveBeenCalledWith("backup_restore");
  });

  it("deletes existing backup artifacts and reconciles backup rows", async () => {
    const deleteBackupArtifact = vi.fn(async () => undefined);
    const execute = vi.fn(async () => 1);
    const reconcileBackupRows = vi.fn(async () => undefined);
    const { workflow } = createWorkflow({
      deleteBackupArtifact,
      execute,
      reconcileBackupRows,
      queryOne: vi.fn(async () => ({
        id: "backup-1",
        game_id: "game-1",
        backup_path: "C:\\Backups\\backup-1",
        backup_size: 42,
        created_at: "2026-04-24T10:00:00.000Z",
        is_auto: 0,
        notes: null,
      })),
    });

    await workflow.deleteBackup("backup-1");

    expect(deleteBackupArtifact).toHaveBeenCalledWith({
      backupPath: "C:\\Backups\\backup-1",
    });
    expect(execute).toHaveBeenCalledWith(
      db,
      "DELETE FROM backups WHERE id = ?1",
      ["backup-1"],
    );
    expect(reconcileBackupRows).toHaveBeenCalledWith(
      db,
      "game-1",
      Number.MAX_SAFE_INTEGER,
    );
  });

  it("checks backup-before-launch using persisted setting and game state", async () => {
    const { workflow } = createWorkflow({
      getGameBackupState: vi.fn(async () => ({
        id: "game-1",
        name: "Arcadia",
        released: null,
        save_path: null,
        backup_enabled: 1,
      })),
      getSetting: vi.fn(async () => "true"),
    });

    await expect(workflow.shouldBackupBeforeLaunch("game-1")).resolves.toBe(true);
  });

  it("maps backup-needed inputs from save discovery and latest backup", async () => {
    const evaluateBackupNeeded = vi.fn(async () => true);
    const currentSave = {
      roots: [{ label: "root-0", path: "C:\\Saves" }],
      files: [
        {
          path: "C:\\Saves\\slot1.sav",
          rootLabel: "root-0",
          relativePath: "slot1.sav",
          size: 42,
        },
      ],
      totalSize: 42,
    };
    const { workflow } = createWorkflow({
      evaluateBackupNeeded,
      getGameBackupState: vi.fn(async () => ({
        id: "game-1",
        name: "Arcadia",
        released: null,
        save_path: "C:\\Saves",
        backup_enabled: 1,
      })),
      loadManifestFromCache: vi.fn(async () => null),
      resolveGameOverridePath: vi.fn(async () => "C:\\Saves"),
      findGameSaveArtifacts: vi.fn(async () => currentSave),
      getLatestBackup: vi.fn(async () => ({
        id: "backup-1",
        game_id: "game-1",
        backup_path: "C:\\Backups\\backup-1",
        backup_size: 40,
        created_at: "2026-04-24T09:00:00.000Z",
        is_auto: true,
        notes: null,
      })),
    });

    await expect(
      workflow.checkBackupNeeded({
        gameId: "game-1",
        gameName: "Arcadia",
      }),
    ).resolves.toBe(true);

    expect(evaluateBackupNeeded).toHaveBeenCalledWith({
      currentSave,
      lastBackup: {
        id: "backup-1",
        gameId: "game-1",
        backupPath: "C:\\Backups\\backup-1",
        backupSize: 40,
        createdAt: "2026-04-24T09:00:00.000Z",
        isAuto: true,
        notes: null,
      },
    });
  });

  it("maps restore-needed results to the IPC response shape", async () => {
    const { workflow } = createWorkflow({
      evaluateRestoreNeeded: vi.fn(async () => ({
        shouldRestore: true,
        backupId: "backup-1",
        currentSize: 12,
        backupSize: 24,
      })),
      getGameBackupState: vi.fn(async () => ({
        id: "game-1",
        name: "Arcadia",
        released: null,
        save_path: "C:\\Saves",
        backup_enabled: 1,
      })),
      loadManifestFromCache: vi.fn(async () => null),
      resolveGameOverridePath: vi.fn(async () => "C:\\Saves"),
      findGameSaveArtifacts: vi.fn(async () => null),
      getLatestBackup: vi.fn(async () => ({
        id: "backup-1",
        game_id: "game-1",
        backup_path: "C:\\Backups\\backup-1",
        backup_size: 24,
        created_at: "2026-04-24T09:00:00.000Z",
        is_auto: true,
        notes: null,
      })),
    });

    await expect(
      workflow.checkRestoreNeeded({
        gameId: "game-1",
        gameName: "Arcadia",
      }),
    ).resolves.toEqual({
      should_restore: true,
      backup_id: "backup-1",
      current_size: 12,
      backup_size: 24,
    });
  });

  it("lists and updates backup settings through the repository boundary", async () => {
    const execute = vi.fn(async () => 1);
    const { workflow } = createWorkflow({
      execute,
      queryAll: vi.fn(async () => [
        ["backup_before_launch", "true"],
        ["max_backups_per_game", "5"],
      ].map(([key, value]) => ({ key, value }))),
      runInTransaction: vi.fn(async (_db, fn) => await fn(db)),
    });

    await expect(workflow.getBackupSettings()).resolves.toEqual({
      backup_before_launch: "true",
      max_backups_per_game: "5",
    });

    await workflow.updateBackupSettings({
      backup_before_launch: "false",
      max_backups_per_game: "3",
    });

    expect(execute).toHaveBeenCalledTimes(2);
    expect(execute).toHaveBeenCalledWith(
      db,
      "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
      ["backup_before_launch", "false"],
    );
    expect(execute).toHaveBeenCalledWith(
      db,
      "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
      ["max_backups_per_game", "3"],
    );
  });
});
