import { defineStore } from "pinia";
import { computed, shallowRef } from "vue";
import { gamesApi } from "../../src/lib/api";
import type { Game, NewGame, UpdateGame } from "../../src/types";

export const useGamesStore = defineStore("arrancador-games", () => {
  const games = shallowRef<Game[]>([]);
  const loading = shallowRef(true);
  const error = shallowRef<string | null>(null);

  const favorites = computed(() => games.value.filter((game) => game.is_favorite));

  const refreshGames = async () => {
    loading.value = true;
    error.value = null;

    try {
      games.value = await gamesApi.getAll();
    } catch (cause) {
      console.error("Failed to load games:", cause);
      error.value = cause instanceof Error ? cause.message : "Failed to load games";
    } finally {
      loading.value = false;
    }
  };

  const addGame = async (game: NewGame) => {
    const created = await gamesApi.add(game);
    games.value = [...games.value, created].sort((left, right) =>
      left.name.localeCompare(right.name),
    );
    return created;
  };

  const addGames = async (items: NewGame[]) => {
    const created = await gamesApi.addBatch(items);
    games.value = [...games.value, ...created].sort((left, right) =>
      left.name.localeCompare(right.name),
    );
    return created;
  };

  const updateGame = async (id: string, updates: Omit<UpdateGame, "id">) => {
    const updated = await gamesApi.update({ id, ...updates });
    games.value = games.value.map((game) => (game.id === id ? updated : game));
    return updated;
  };

  const deleteGame = async (id: string) => {
    await gamesApi.delete(id);
    games.value = games.value.filter((game) => game.id !== id);
  };

  const toggleFavorite = async (id: string) => {
    const updated = await gamesApi.toggleFavorite(id);
    games.value = games.value.map((game) => (game.id === id ? updated : game));
    return updated;
  };

  const searchGames = async (query: string) => {
    if (!query.trim()) {
      return games.value;
    }
    return gamesApi.search(query);
  };

  const getGame = (id: string) => games.value.find((game) => game.id === id);

  return {
    games,
    loading,
    error,
    favorites,
    refreshGames,
    addGame,
    addGames,
    updateGame,
    deleteGame,
    toggleFavorite,
    searchGames,
    getGame,
  };
});
