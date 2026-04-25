import { describe, expect, it, vi } from "vitest";
import type { RuntimeState } from "../backend";
import { registerBackupIpcHandlers } from "./backup-handlers";

const mocks = vi.hoisted(() => ({
  handle: vi.fn(),
  createBackupWorkflow: vi.fn(),
}));

vi.mock("electron", () => ({
  ipcMain: {
    handle: mocks.handle,
  },
}));

vi.mock("../services/backup-workflow", () => ({
  createBackupWorkflow: mocks.createBackupWorkflow,
}));

describe("registerBackupIpcHandlers", () => {
  it("delegates write IPC handlers through the backup workflow", async () => {
    const workflow = {
      checkLudusaviInstalled: vi.fn(),
      getLudusaviExecutablePath: vi.fn(),
      setLudusaviPath: vi.fn(),
      setBackupDirectory: vi.fn(),
      getBackupDirectorySetting: vi.fn(),
      refreshSqobaManifest: vi.fn(),
      findGameSavePaths: vi.fn(),
      findGameSaves: vi.fn(),
      createBackup: vi.fn(async () => ({ id: "backup-1" })),
      getGameBackups: vi.fn(),
      restoreBackup: vi.fn(async () => undefined),
      deleteBackup: vi.fn(async () => undefined),
      shouldBackupBeforeLaunch: vi.fn(),
      checkBackupNeeded: vi.fn(),
      checkRestoreNeeded: vi.fn(),
      getBackupSettings: vi.fn(),
      updateBackupSettings: vi.fn(),
    };
    mocks.createBackupWorkflow.mockReturnValue(workflow);
    const runtime = {
      db: {},
      services: {
        settings: { getAllSettings: vi.fn() },
        achievements: { recordAchievementEvent: vi.fn() },
      },
    } as unknown as RuntimeState;
    const withRuntime = vi.fn((handler) => async (_event: unknown, payload: unknown) =>
      handler(runtime, payload),
    );
    const emitRendererEvent = vi.fn();

    registerBackupIpcHandlers({
      withRuntime,
      getUserDataPath: () => "C:\\Users\\Kazui\\AppData\\Roaming\\arrancador",
      emitRendererEvent,
    });

    const handlers = new Map(
      mocks.handle.mock.calls.map(([channel, handler]) => [channel, handler]),
    );

    await expect(
      handlers.get("create_backup")?.(null, {
        gameId: "game-1",
        gameName: "Arcadia",
        isAuto: false,
      }),
    ).resolves.toEqual({ id: "backup-1" });
    await expect(
      handlers.get("restore_backup")?.(null, { backupId: "backup-1" }),
    ).resolves.toBeUndefined();
    await expect(
      handlers.get("delete_backup")?.(null, { backupId: "backup-1" }),
    ).resolves.toBeUndefined();

    expect(workflow.createBackup).toHaveBeenCalledWith({
      gameId: "game-1",
      gameName: "Arcadia",
      isAuto: false,
    });
    expect(workflow.restoreBackup).toHaveBeenCalledWith("backup-1");
    expect(workflow.deleteBackup).toHaveBeenCalledWith("backup-1");
    expect(mocks.createBackupWorkflow).toHaveBeenCalledWith({
      db: runtime.db,
      userDataPath: "C:\\Users\\Kazui\\AppData\\Roaming\\arrancador",
      settings: runtime.services.settings,
      achievements: runtime.services.achievements,
      emitRendererEvent,
    });
  });
});
