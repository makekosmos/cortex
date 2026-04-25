import { describe, expect, it, vi } from "vitest";
import { shallowRef } from "vue";
import { createTestGame } from "../../src/types";
import {
  type GameLaunchFlowApis,
  useGameLaunchFlow,
} from "../composables/useGameLaunchFlow";

function createApis(overrides: Partial<GameLaunchFlowApis> = {}) {
  return {
    launchGame: vi.fn(async () => undefined),
    getRunningInstances: vi.fn(async () => 0),
    killProcesses: vi.fn(async () => 1),
    checkRestoreNeeded: vi.fn(async () => ({
      should_restore: false,
      backup_id: null,
      current_size: 0,
      backup_size: 0,
    })),
    restoreBackup: vi.fn(async () => undefined),
    shouldBackupBeforeLaunch: vi.fn(async () => false),
    checkBackupNeeded: vi.fn(async () => false),
    createBackup: vi.fn(async () => undefined),
    ...overrides,
  } satisfies GameLaunchFlowApis;
}

function createHarness(options: {
  running?: number;
  backupEnabled?: boolean;
  apis?: Partial<GameLaunchFlowApis>;
  confirm?: (message: string) => boolean;
} = {}) {
  const game = shallowRef(
    createTestGame({
      backup_enabled: options.backupEnabled ?? true,
    }),
  );
  const runningCount = shallowRef(options.running ?? 0);
  const restoring = shallowRef(false);
  const creatingBackup = shallowRef(false);
  const refreshGames = vi.fn(async () => undefined);
  const loadBackups = vi.fn(async () => undefined);
  const notify = vi.fn();
  const setRunningCount = vi.fn((count: number) => {
    runningCount.value = count;
  });
  const apis = createApis(options.apis);

  const flow = useGameLaunchFlow({
    game: () => game.value,
    isMissing: () => false,
    runningCount,
    restoring,
    creatingBackup,
    setRunningCount,
    refreshGames,
    loadBackups,
    notify,
    apis,
    confirm: options.confirm ?? (() => true),
    log: {
      error: vi.fn(),
    },
  });

  return {
    flow,
    game,
    runningCount,
    restoring,
    creatingBackup,
    refreshGames,
    loadBackups,
    notify,
    setRunningCount,
    apis,
  };
}

describe("useGameLaunchFlow", () => {
  it("closes a running game instead of launching another instance", async () => {
    const harness = createHarness({
      running: 2,
      apis: {
        getRunningInstances: vi.fn(async () => 0),
      },
    });

    await harness.flow.handleLaunch();

    expect(harness.apis.killProcesses).toHaveBeenCalledWith("game-1");
    expect(harness.apis.getRunningInstances).toHaveBeenCalledWith("game-1");
    expect(harness.setRunningCount).toHaveBeenCalledWith(0);
    expect(harness.apis.launchGame).not.toHaveBeenCalled();
    expect(harness.runningCount.value).toBe(0);
  });

  it("restores the latest backup before launch when the restore check requests it", async () => {
    const harness = createHarness({
      apis: {
        checkRestoreNeeded: vi.fn(async () => ({
          should_restore: true,
          backup_id: "backup-1",
          current_size: 12,
          backup_size: 24,
        })),
      },
    });

    await harness.flow.handleLaunch();

    expect(harness.apis.restoreBackup).toHaveBeenCalledWith("backup-1");
    expect(harness.restoring.value).toBe(false);
    expect(harness.apis.launchGame).toHaveBeenCalledWith("game-1");
    expect(harness.refreshGames).toHaveBeenCalledTimes(2);
  });

  it("creates an automatic backup before launch when saves changed", async () => {
    const harness = createHarness({
      apis: {
        shouldBackupBeforeLaunch: vi.fn(async () => true),
        checkBackupNeeded: vi.fn(async () => true),
      },
    });

    await harness.flow.handleLaunch();

    expect(harness.apis.createBackup).toHaveBeenCalledWith(
      "game-1",
      "Arcadia",
      true,
    );
    expect(harness.loadBackups).toHaveBeenCalledTimes(1);
    expect(harness.creatingBackup.value).toBe(false);
    expect(harness.apis.launchGame).toHaveBeenCalledWith("game-1");
  });

  it("notifies when launch fails", async () => {
    const harness = createHarness({
      backupEnabled: false,
      apis: {
        launchGame: vi.fn(async () => {
          throw new Error("spawn failed");
        }),
      },
    });

    await harness.flow.handleLaunch();

    expect(harness.notify).toHaveBeenCalledWith({
      tone: "error",
      title: "Не удалось запустить игру",
    });
    expect(harness.flow.launching.value).toBe(false);
  });
});
