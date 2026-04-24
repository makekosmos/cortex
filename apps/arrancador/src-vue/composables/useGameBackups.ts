import { computed, reactive, shallowRef, watch } from "vue";
import { backupApi } from "../../src/lib/api";
import { subscribeAppEvent } from "../../src/lib/browser";
import type { Backup, Game } from "../../src/types";

type BackupProgressPayload = {
  game_id: string;
  stage: string;
  message: string;
  done: number;
  total: number;
};

type Notify = (input: {
  tone: "success" | "warning" | "error";
  title: string;
  description?: string;
}) => void;

export interface GameBackupsOptions {
  game: () => Game | null;
  refreshGames: () => Promise<void>;
  notify: Notify;
  confirm?: (message: string) => boolean;
  log?: Pick<Console, "error">;
}

function sortBackups(backups: readonly Backup[]) {
  return [...backups].sort(
    (left, right) =>
      new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
  );
}

export function useGameBackups(options: GameBackupsOptions) {
  const backups = shallowRef<Backup[]>([]);
  const loadingBackups = shallowRef(false);
  const restoring = shallowRef(false);
  const creatingBackup = shallowRef(false);
  const showAllBackups = shallowRef(false);
  const confirm = options.confirm ?? window.confirm;
  const log = options.log ?? console;

  const backupProgress = reactive({
    active: false,
    stage: "",
    message: "",
    done: 0,
    total: 0,
  });

  const latestBackup = computed(() => backups.value[0] ?? null);
  const olderBackups = computed(() => backups.value.slice(1));

  async function loadBackups() {
    const currentGame = options.game();
    if (!currentGame) return;

    loadingBackups.value = true;
    try {
      backups.value = sortBackups(await backupApi.getForGame(currentGame.id));
    } catch (cause) {
      log.error?.("Failed to load backups:", cause);
    } finally {
      loadingBackups.value = false;
    }
  }

  async function createManualBackup() {
    const currentGame = options.game();
    if (!currentGame) return;

    creatingBackup.value = true;
    try {
      await backupApi.create(currentGame.id, currentGame.name, false);
      await loadBackups();
      await options.refreshGames();
      options.notify({
        tone: "success",
        title: "Бэкап создан",
      });
    } catch (cause) {
      log.error?.("Backup failed:", cause);
      options.notify({
        tone: "error",
        title: "Не удалось создать бэкап",
      });
    } finally {
      creatingBackup.value = false;
    }
  }

  async function restoreBackup(backupId: string) {
    if (
      !confirm(
        "Восстановить бэкап? Текущие сохранения будут перезаписаны.",
      )
    ) {
      return;
    }

    restoring.value = true;
    try {
      await backupApi.restore(backupId);
      await loadBackups();
      await options.refreshGames();
      options.notify({
        tone: "success",
        title: "Бэкап восстановлен",
      });
    } catch (cause) {
      log.error?.("Restore failed:", cause);
      options.notify({
        tone: "error",
        title: "Не удалось восстановить бэкап",
      });
    } finally {
      restoring.value = false;
    }
  }

  watch(
    () => options.game()?.id,
    async (currentId) => {
      if (!currentId) {
        backups.value = [];
        return;
      }

      await loadBackups();
    },
    { immediate: true },
  );

  watch(
    () => options.game()?.id,
    (currentId, _previousId, onCleanup) => {
      if (!currentId) return;

      const syncProgress = (payload: BackupProgressPayload) => {
        if (payload.game_id !== currentId) return;
        backupProgress.stage = payload.stage;
        backupProgress.message = payload.message;
        backupProgress.done = payload.done;
        backupProgress.total = payload.total;
        backupProgress.active = payload.stage !== "done";
      };

      const stopBackup = subscribeAppEvent<BackupProgressPayload>(
        "backup:progress",
        syncProgress,
      );
      const stopRestore = subscribeAppEvent<BackupProgressPayload>(
        "restore:progress",
        syncProgress,
      );

      onCleanup(() => {
        stopBackup();
        stopRestore();
      });
    },
    { immediate: true },
  );

  return {
    backups,
    loadingBackups,
    restoring,
    creatingBackup,
    showAllBackups,
    backupProgress,
    latestBackup,
    olderBackups,
    loadBackups,
    createManualBackup,
    restoreBackup,
  };
}
