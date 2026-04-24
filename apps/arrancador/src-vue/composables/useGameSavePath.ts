import { computed, shallowRef, watch } from "vue";
import { backupApi } from "../../src/lib/api";
import {
  openPath,
  pickDirectoryPath,
  pickFilePath,
} from "../../src/lib/browser";
import type { Game } from "../../src/types";
import {
  GAME_PATH_TOKEN,
  resolveSavePathTemplate,
} from "../lib/gameDetailDisplay";

type Notify = (input: {
  tone: "success" | "warning" | "error";
  title: string;
  description?: string;
}) => void;

export interface GameSavePathOptions {
  game: () => Game | null;
  updateGame: (id: string, updates: { save_path: string | null }) => Promise<unknown>;
  notify: Notify;
  log?: Pick<Console, "error">;
}

export function useGameSavePath(options: GameSavePathOptions) {
  const savingPath = shallowRef(false);
  const locatingSavePath = shallowRef(false);
  const savePathDraft = shallowRef("");
  const log = options.log ?? console;

  const savePathPreviewParts = computed(() =>
    savePathDraft.value.split(GAME_PATH_TOKEN),
  );
  const canOpenSavePath = computed(() => savePathDraft.value.trim().length > 0);

  watch(
    () => options.game()?.save_path,
    (savePath) => {
      savePathDraft.value = savePath || "";
    },
    { immediate: true },
  );

  async function locateSavePath() {
    const currentGame = options.game();
    if (!currentGame) return;

    locatingSavePath.value = true;
    try {
      const info = await backupApi.findGameSavePaths(
        currentGame.name,
        currentGame.id,
      );
      if (info.save_path) {
        savePathDraft.value = info.save_path;
        options.notify({
          tone: "success",
          title: "Путь найден",
        });
      } else {
        options.notify({
          tone: "warning",
          title: "Сохранения не найдены",
          description: "Укажите путь вручную.",
        });
      }
    } catch (cause) {
      log.error?.("Failed to locate saves:", cause);
      options.notify({
        tone: "error",
        title: "Ошибка поиска сохранений",
      });
    } finally {
      locatingSavePath.value = false;
    }
  }

  async function openSavePath() {
    const currentGame = options.game();
    if (!currentGame || !savePathDraft.value.trim()) return;
    await openPath(resolveSavePathTemplate(savePathDraft.value.trim(), currentGame));
  }

  async function chooseSaveFolder() {
    const selected = await pickDirectoryPath({
      title: "Выбрать папку с сохранениями",
    });
    if (selected) {
      savePathDraft.value = selected;
    }
  }

  async function chooseSaveFile() {
    const selected = await pickFilePath({
      title: "Выбрать файл сохранения",
    });
    if (selected) {
      savePathDraft.value = selected;
    }
  }

  function insertGamePathToken() {
    const current = savePathDraft.value.trim();
    if (current.startsWith(GAME_PATH_TOKEN)) return;
    const needsSeparator =
      current.length > 0 && !current.startsWith("\\") && !current.startsWith("/");
    savePathDraft.value = `${GAME_PATH_TOKEN}${needsSeparator ? "\\" : ""}${current}`;
  }

  async function saveGamePath() {
    const currentGame = options.game();
    if (!currentGame) return;

    savingPath.value = true;
    try {
      const normalizedPath = savePathDraft.value.trim();
      await options.updateGame(currentGame.id, {
        save_path: normalizedPath === "" ? null : normalizedPath,
      });
      options.notify({
        tone: "success",
        title: "Путь сохранен",
      });
    } catch (cause) {
      log.error?.("Failed to save game path:", cause);
      options.notify({
        tone: "error",
        title: "Не удалось сохранить путь",
      });
    } finally {
      savingPath.value = false;
    }
  }

  return {
    savingPath,
    locatingSavePath,
    savePathDraft,
    savePathPreviewParts,
    canOpenSavePath,
    locateSavePath,
    openSavePath,
    chooseSaveFolder,
    chooseSaveFile,
    insertGamePathToken,
    saveGamePath,
  };
}
