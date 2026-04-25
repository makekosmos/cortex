import { type Ref, shallowRef } from "vue";
import { backupApi, gamesApi } from "../../src/lib/api";
import type { Game, RestoreCheck } from "../../src/types";
import { formatBytes } from "../lib/gameDetailDisplay";

type Notify = (input: {
  tone: "success" | "warning" | "error";
  title: string;
  description?: string;
}) => void;

export interface GameLaunchFlowApis {
  launchGame(id: string): Promise<void>;
  getRunningInstances(id: string): Promise<number>;
  killProcesses(id: string): Promise<number>;
  checkRestoreNeeded(id: string, name: string): Promise<RestoreCheck>;
  restoreBackup(backupId: string): Promise<void>;
  shouldBackupBeforeLaunch(id: string): Promise<boolean>;
  checkBackupNeeded(id: string, name: string): Promise<boolean>;
  createBackup(id: string, name: string, isAuto: boolean): Promise<unknown>;
}

export interface GameLaunchFlowOptions {
  game: () => Game | null;
  isMissing: () => boolean;
  runningCount: Ref<number>;
  restoring: Ref<boolean>;
  creatingBackup: Ref<boolean>;
  setRunningCount: (count: number) => void;
  refreshGames: () => Promise<void>;
  loadBackups: () => Promise<void>;
  notify: Notify;
  apis?: Partial<GameLaunchFlowApis>;
  confirm?: (message: string) => boolean;
  log?: Pick<Console, "error">;
}

function createDefaultApis(): GameLaunchFlowApis {
  return {
    launchGame: (id) => gamesApi.launch(id),
    getRunningInstances: (id) => gamesApi.getRunningInstances(id),
    killProcesses: (id) => gamesApi.killProcesses(id),
    checkRestoreNeeded: (id, name) => backupApi.checkRestoreNeeded(id, name),
    restoreBackup: (backupId) => backupApi.restore(backupId),
    shouldBackupBeforeLaunch: (id) => backupApi.shouldBackupBeforeLaunch(id),
    checkBackupNeeded: (id, name) => backupApi.checkBackupNeeded(id, name),
    createBackup: (id, name, isAuto) => backupApi.create(id, name, isAuto),
  };
}

export function useGameLaunchFlow(options: GameLaunchFlowOptions) {
  const launching = shallowRef(false);
  const apis = {
    ...createDefaultApis(),
    ...options.apis,
  };
  const confirm = options.confirm ?? window.confirm;
  const log = options.log ?? console;

  async function launchGame() {
    const currentGame = options.game();
    if (!currentGame) return;

    launching.value = true;

    try {
      await apis.launchGame(currentGame.id);
      const count = await apis.getRunningInstances(currentGame.id);
      options.setRunningCount(count);
      await options.refreshGames();
    } catch (cause) {
      log.error?.("Failed to launch game:", cause);
      options.notify({
        tone: "error",
        title: "Не удалось запустить игру",
      });
    } finally {
      launching.value = false;
    }
  }

  async function stopRunningGame(currentGame: Game) {
    const confirmed = confirm(
      options.runningCount.value > 1
        ? `Игра уже запущена. Закрыть ${options.runningCount.value} процесса?`
        : "Игра уже запущена. Закрыть игру?",
    );

    if (!confirmed) return;

    try {
      await apis.killProcesses(currentGame.id);
      const remaining = await apis.getRunningInstances(currentGame.id);
      options.setRunningCount(remaining);
      if (remaining > 0) {
        options.notify({
          tone: "warning",
          title: "Не удалось закрыть все процессы",
          description: `Осталось процессов: ${remaining}`,
        });
      }
    } catch (cause) {
      log.error?.("Failed to kill processes:", cause);
      options.notify({
        tone: "error",
        title: "Не удалось закрыть игру",
      });
    }
  }

  async function restoreBeforeLaunch(currentGame: Game) {
    const restoreCheck = await apis.checkRestoreNeeded(
      currentGame.id,
      currentGame.name,
    );
    if (!restoreCheck.should_restore || !restoreCheck.backup_id) {
      return;
    }

    const shouldRestore = confirm(
      `Текущий размер сохранений меньше, чем в бэкапе. Восстановить перед запуском?\n\nТекущий: ${formatBytes(restoreCheck.current_size)} • Бэкап: ${formatBytes(restoreCheck.backup_size)}`,
    );

    if (!shouldRestore) return;

    options.restoring.value = true;
    try {
      await apis.restoreBackup(restoreCheck.backup_id);
      await options.refreshGames();
    } finally {
      options.restoring.value = false;
    }
  }

  async function backupBeforeLaunch(currentGame: Game) {
    const shouldBackup = await apis.shouldBackupBeforeLaunch(currentGame.id);
    if (!shouldBackup) {
      return;
    }

    const needsBackup = await apis.checkBackupNeeded(
      currentGame.id,
      currentGame.name,
    );
    if (!needsBackup) {
      return;
    }

    const shouldCreateBackup = confirm(
      "Сохранения изменились. Создать бэкап перед запуском?",
    );
    if (!shouldCreateBackup) {
      return;
    }

    options.creatingBackup.value = true;
    try {
      await apis.createBackup(currentGame.id, currentGame.name, true);
      await options.loadBackups();
      await options.refreshGames();
    } finally {
      options.creatingBackup.value = false;
    }
  }

  async function prepareLaunch(currentGame: Game): Promise<boolean> {
    if (!currentGame.backup_enabled) {
      return true;
    }

    try {
      await restoreBeforeLaunch(currentGame);
      await backupBeforeLaunch(currentGame);
      return true;
    } catch (cause) {
      log.error?.("Backup or restore pre-launch flow failed:", cause);
      options.creatingBackup.value = false;
      options.restoring.value = false;
      options.notify({
        tone: "error",
        title: "Не удалось подготовить запуск",
      });
      return false;
    }
  }

  async function handleLaunch() {
    const currentGame = options.game();
    if (!currentGame || options.isMissing()) return;

    if (options.runningCount.value > 0) {
      await stopRunningGame(currentGame);
      return;
    }

    const canLaunch = await prepareLaunch(currentGame);
    if (!canLaunch) return;

    await launchGame();
  }

  return {
    launching,
    handleLaunch,
    launchGame,
  };
}
