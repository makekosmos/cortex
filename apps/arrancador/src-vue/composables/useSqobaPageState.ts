import { computed, onMounted, reactive, shallowRef } from "vue";
import { backupApi } from "../../src/lib/api";
import { openPath, pickDirectoryPath, pickFilePath } from "../../src/lib/browser";
import type { BackupInfo, Game, SavePathLookup } from "../../src/types";
import { GAME_PATH_TOKEN } from "../components/sqoba/types";
import type { LookupState } from "../components/sqoba/types";
import { useGamesStore } from "../stores/games";
import { useSettingsPageState } from "./useSettingsPageState";
import { useToast } from "./useToast";

function resolveSavePathTemplate(path: string, game?: Game | null) {
  if (!path.includes(GAME_PATH_TOKEN)) {
    return path;
  }

  const exePath = game?.exe_path;
  if (!exePath) {
    return path;
  }

  const lastSlash = Math.max(exePath.lastIndexOf("\\"), exePath.lastIndexOf("/"));
  const base = lastSlash > 0 ? exePath.slice(0, lastSlash) : exePath;
  return path.split(GAME_PATH_TOKEN).join(base);
}

export function useSqobaPageState() {
  const gamesStore = useGamesStore();
  const settingsState = useSettingsPageState();
  const { notify } = useToast();

  const aboutOpen = shallowRef(false);
  const query = shallowRef("");
  const onlyMissing = shallowRef(false);
  const scanAllLoading = shallowRef(false);
  const editingGameId = shallowRef<string | null>(null);
  const savePathDraft = shallowRef("");
  const savingSavePath = shallowRef(false);

  const pathsByGameId = reactive<Record<string, LookupState<SavePathLookup>>>({});
  const filesByGameId = reactive<Record<string, LookupState<BackupInfo | null>>>({});

  const sortedGames = computed(() =>
    [...gamesStore.games].sort((left, right) => left.name.localeCompare(right.name)),
  );

  const filteredGames = computed(() => {
    const needle = query.value.trim().toLowerCase();

    return sortedGames.value.filter((game) => {
      if (onlyMissing.value && game.save_path) {
        return false;
      }

      if (!needle) {
        return true;
      }

      return game.name.toLowerCase().includes(needle);
    });
  });

  const missingCount = computed(() => gamesStore.games.filter((game) => !game.save_path).length);

  async function ensureGamesLoaded() {
    if (gamesStore.games.length > 0 || gamesStore.loading) {
      return;
    }

    await gamesStore.refreshGames();
  }

  async function openGamePath(game: Game, path: string) {
    try {
      await openPath(resolveSavePathTemplate(path, game));
    } catch (error) {
      console.error("Failed to open SQOBA path:", error);
      notify({
        tone: "error",
        title: "Failed to open path",
        description: "Verify that the path exists and is accessible.",
      });
    }
  }

  async function loadSavePaths(game: Game) {
    pathsByGameId[game.id] = { loading: true };

    try {
      const result = await backupApi.findGameSavePaths(game.name, game.id);
      pathsByGameId[game.id] = {
        loading: false,
        data: result,
      };

      if (!result.save_path) {
        notify({
          tone: "warning",
          title: "Save path not found",
          description: `SQOBA could not detect a save path for "${game.name}".`,
        });
        return;
      }

      if (result.candidates.length > 1) {
        notify({
          tone: "info",
          title: "Multiple candidates found",
          description: "Review the candidate paths or set the save path manually.",
        });
      }
    } catch (error) {
      console.error("Failed to locate save paths:", error);
      pathsByGameId[game.id] = {
        loading: false,
        error: error instanceof Error ? error.message : "Save-path lookup failed.",
      };
      notify({
        tone: "error",
        title: "Save lookup failed",
        description: `Could not search for saves for "${game.name}".`,
      });
    }
  }

  async function loadSaveFiles(game: Game) {
    filesByGameId[game.id] = { loading: true };

    try {
      const result = await backupApi.findGameSaves(game.name, game.id);
      filesByGameId[game.id] = {
        loading: false,
        data: result,
      };

      if (!result?.save_path) {
        notify({
          tone: "warning",
          title: "Save files not found",
          description: "Set the save path manually if the game uses a non-standard location.",
        });
      }
    } catch (error) {
      console.error("Failed to load save files:", error);
      filesByGameId[game.id] = {
        loading: false,
        error: error instanceof Error ? error.message : "Save-file lookup failed.",
      };
      notify({
        tone: "error",
        title: "File lookup failed",
        description: `Could not load save files for "${game.name}".`,
      });
    }
  }

  function beginEdit(game: Game) {
    editingGameId.value = game.id;
    savePathDraft.value = game.save_path ?? pathsByGameId[game.id]?.data?.save_path ?? "";
  }

  function cancelEdit() {
    editingGameId.value = null;
    savePathDraft.value = "";
  }

  async function selectSaveFolder() {
    const selectedPath = await pickDirectoryPath({
      title: "Select a save folder",
    });

    if (selectedPath) {
      savePathDraft.value = selectedPath;
    }
  }

  async function selectSaveFile() {
    const selectedPath = await pickFilePath({
      title: "Select a save file",
    });

    if (selectedPath) {
      savePathDraft.value = selectedPath;
    }
  }

  function insertGamePathToken() {
    const current = savePathDraft.value.trim();
    if (current.startsWith(GAME_PATH_TOKEN)) {
      return;
    }

    const needsSeparator =
      current.length > 0 && !current.startsWith("\\") && !current.startsWith("/");

    savePathDraft.value = `${GAME_PATH_TOKEN}${needsSeparator ? "\\" : ""}${current}`;
  }

  async function saveGameSavePath(game: Game) {
    if (savingSavePath.value) {
      return;
    }

    savingSavePath.value = true;

    try {
      const trimmed = savePathDraft.value.trim();
      const value = trimmed.length > 0 ? trimmed : null;
      await gamesStore.updateGame(game.id, { save_path: value });

      if (pathsByGameId[game.id]?.data) {
        pathsByGameId[game.id] = {
          ...pathsByGameId[game.id],
          data: {
            ...pathsByGameId[game.id].data!,
            save_path: value,
          },
        };
      }

      delete filesByGameId[game.id];

      notify({
        tone: "success",
        title: "Save path updated",
      });
      cancelEdit();
    } catch (error) {
      console.error("Failed to update save path:", error);
      notify({
        tone: "error",
        title: "Save path update failed",
        description: error instanceof Error ? error.message : "Unknown error",
      });
    } finally {
      savingSavePath.value = false;
    }
  }

  async function runScanAll() {
    if (scanAllLoading.value) {
      return;
    }

    scanAllLoading.value = true;

    try {
      for (const game of filteredGames.value) {
        if (pathsByGameId[game.id]?.data || pathsByGameId[game.id]?.loading) {
          continue;
        }

        // Sequential scans keep toast feedback and backend load predictable.
        // eslint-disable-next-line no-await-in-loop
        await loadSavePaths(game);
      }
    } finally {
      scanAllLoading.value = false;
    }
  }

  async function refreshManifestAndGames() {
    await settingsState.refreshSqobaManifest();

    if (settingsState.manifestFeedback.value?.tone === "success") {
      await gamesStore.refreshGames();
    }
  }

  function openAbout() {
    aboutOpen.value = true;
  }

  function closeAbout() {
    aboutOpen.value = false;
  }

  onMounted(() => {
    void ensureGamesLoaded();
  });

  return {
    settingsState,
    aboutOpen,
    query,
    onlyMissing,
    scanAllLoading,
    editingGameId,
    savePathDraft,
    savingSavePath,
    pathsByGameId,
    filesByGameId,
    games: computed(() => gamesStore.games),
    gamesLoading: computed(() => gamesStore.loading),
    gamesError: computed(() => gamesStore.error),
    filteredGames,
    missingCount,
    openAbout,
    closeAbout,
    openGamePath,
    loadSavePaths,
    loadSaveFiles,
    beginEdit,
    cancelEdit,
    selectSaveFolder,
    selectSaveFile,
    insertGamePathToken,
    saveGameSavePath,
    runScanAll,
    refreshManifestAndGames,
  };
}
