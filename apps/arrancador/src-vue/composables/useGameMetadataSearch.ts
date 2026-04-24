import { reactive, shallowRef, watch } from "vue";
import { metadataApi } from "../../src/lib/api";
import { openPath } from "../../src/lib/browser";
import type { Game, RawgGame } from "../../src/types";

export interface GameMetadataSearchOptions {
  game: () => Game | null;
  updateGame: (
    id: string,
    updates: {
      name: string;
      description: string;
      background_image: string;
      cover_image: string;
    },
  ) => Promise<unknown>;
  refreshGames: () => Promise<void>;
  log?: Pick<Console, "error">;
}

export function useGameMetadataSearch(options: GameMetadataSearchOptions) {
  const searchingMetadata = shallowRef(false);
  const applyingMetadata = shallowRef(false);
  const savingEdit = shallowRef(false);
  const showMetadataSearch = shallowRef(false);
  const showEditDialog = shallowRef(false);
  const metadataQuery = shallowRef("");
  const metadataResults = shallowRef<RawgGame[]>([]);
  const renameFromMetadata = shallowRef(false);
  const log = options.log ?? console;

  const editForm = reactive({
    name: "",
    description: "",
    background_image: "",
    cover_image: "",
  });

  watch(
    options.game,
    (currentGame) => {
      if (!currentGame) return;
      editForm.name = currentGame.name;
      editForm.description = currentGame.description || "";
      editForm.background_image = currentGame.background_image || "";
      editForm.cover_image = currentGame.cover_image || "";
    },
    { immediate: true },
  );

  async function handleSaveEdit() {
    const currentGame = options.game();
    if (!currentGame) return;

    savingEdit.value = true;
    try {
      await options.updateGame(currentGame.id, {
        name: editForm.name,
        description: editForm.description,
        background_image: editForm.background_image,
        cover_image: editForm.cover_image,
      });
      await options.refreshGames();
      showEditDialog.value = false;
    } catch (cause) {
      log.error?.("Failed to update game:", cause);
    } finally {
      savingEdit.value = false;
    }
  }

  async function handleSearchGoogleImage(
    searchQueryText: string,
    target: "background" | "cover",
  ) {
    const suffix = target === "cover" ? " cover" : " background";
    const query = encodeURIComponent(`${searchQueryText}${suffix}`);
    await openPath(`https://www.google.com/search?tbm=isch&q=${query}`);
  }

  async function searchMetadata() {
    if (!metadataQuery.value.trim()) return;

    searchingMetadata.value = true;
    try {
      metadataResults.value = await metadataApi.search(metadataQuery.value.trim());
    } catch (cause) {
      log.error?.("Metadata search failed:", cause);
    } finally {
      searchingMetadata.value = false;
    }
  }

  async function applyMetadata(rawgGame: RawgGame) {
    const currentGame = options.game();
    if (!currentGame) return;

    applyingMetadata.value = true;
    try {
      await metadataApi.apply(
        currentGame.id,
        rawgGame.id,
        renameFromMetadata.value,
      );
      await options.refreshGames();
      showMetadataSearch.value = false;
      metadataResults.value = [];
      metadataQuery.value = "";
    } catch (cause) {
      log.error?.("Failed to apply metadata:", cause);
    } finally {
      applyingMetadata.value = false;
    }
  }

  return {
    searchingMetadata,
    applyingMetadata,
    savingEdit,
    showMetadataSearch,
    showEditDialog,
    metadataQuery,
    metadataResults,
    renameFromMetadata,
    editForm,
    handleSaveEdit,
    handleSearchGoogleImage,
    searchMetadata,
    applyMetadata,
  };
}
