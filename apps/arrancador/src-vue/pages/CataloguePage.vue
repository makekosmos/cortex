<script setup lang="ts">
import { ExternalLink, Gamepad2, Loader2, Plus, Star } from "lucide-vue-next";
import { computed, onMounted, reactive } from "vue";
import { RouterLink } from "vue-router";
import { gamesApi, metadataApi } from "../../src/lib/api";
import { translateGenreListToRu } from "../../src/lib/genres";
import type { RawgGame } from "../../src/types";
import { useToast } from "../composables/useToast";
import { useGamesStore } from "../stores/games";

function sanitizeForExeName(value: string) {
  return Array.from(value, (char) =>
    char.charCodeAt(0) < 32 || '<>:"/\\|?*'.includes(char) ? " " : char,
  )
    .join("")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 80);
}

function normalizeName(value: string) {
  return value.trim().toLowerCase();
}

const gamesStore = useGamesStore();
const { notify } = useToast();

const state = reactive({
  query: "",
  items: [] as RawgGame[],
  loading: false,
  searching: false,
  addingId: null as number | null,
  error: null as string | null,
});

const libraryByRawgId = computed(() => {
  const map = new Map<number, string>();
  for (const game of gamesStore.games) {
    if (game.rawg_id) {
      map.set(game.rawg_id, game.id);
    }
  }
  return map;
});

const libraryByName = computed(() => {
  const map = new Map<string, string>();
  for (const game of gamesStore.games) {
    map.set(normalizeName(game.name), game.id);
  }
  return map;
});

async function loadShowcase(searchValue: string, mode: "initial" | "search" = "search") {
  if (mode === "initial") {
    state.loading = true;
    state.searching = false;
  } else {
    state.searching = true;
    state.loading = false;
  }

  state.error = null;

  try {
    state.items = await metadataApi.search(searchValue.trim());
  } catch (cause) {
    console.error("Failed to load RAWG catalogue:", cause);
    state.error = mode === "initial" ? "Failed to load RAWG showcase" : "Failed to search RAWG";
    if (mode === "search") {
      notify({ tone: "error", title: "RAWG search failed" });
    }
  } finally {
    if (mode === "initial") {
      state.loading = false;
    } else {
      state.searching = false;
    }
  }
}

function isInLibrary(item: RawgGame) {
  return libraryByRawgId.value.has(item.id) || libraryByName.value.has(normalizeName(item.name));
}

function getLibraryGameLink(item: RawgGame) {
  const byRawg = libraryByRawgId.value.get(item.id);
  if (byRawg) return `/game/${byRawg}`;
  const byName = libraryByName.value.get(normalizeName(item.name));
  if (byName) return `/game/${byName}`;
  return null;
}

async function addToLibrary(item: RawgGame) {
  if (isInLibrary(item)) return;

  state.addingId = item.id;
  let createdGameId: string | null = null;

  try {
    const safeName = sanitizeForExeName(item.name) || `rawg-${item.id}`;
    const exeName = `${safeName}.exe`;
    const exePath = `C:\\Arrancador\\RAWG\\${item.id}\\${exeName}`;

    const game = await gamesStore.addGame({
      name: item.name,
      exe_name: exeName,
      exe_path: exePath,
    });
    createdGameId = game.id;

    await metadataApi.apply(game.id, item.id, true);
    await gamesStore.refreshGames();

    notify({
      tone: "success",
      title: "Game added to library",
      description: item.name,
    });
  } catch (cause) {
    console.error("Failed to add game from RAWG:", cause);

    if (createdGameId) {
      try {
        await gamesApi.delete(createdGameId);
        await gamesStore.refreshGames();
      } catch (cleanupCause) {
        console.error("Failed to roll back placeholder game:", cleanupCause);
      }
    }

    notify({
      tone: "error",
      title: "Failed to add game",
      description: item.name,
    });
  } finally {
    state.addingId = null;
  }
}

onMounted(async () => {
  if (gamesStore.games.length === 0 && !gamesStore.loading) {
    await gamesStore.refreshGames();
  }
  await loadShowcase("", "initial");
});
</script>

<template>
  <div class="mx-auto max-w-7xl space-y-6 p-4 sm:p-6">
    <div class="space-y-2">
      <h1 class="text-2xl font-bold">Game Catalogue</h1>
      <p class="text-sm text-muted-foreground">
        Popular games from RAWG. Search and add them to your library.
      </p>
    </div>

    <div class="flex flex-col gap-3 sm:flex-row">
      <input
        v-model="state.query"
        data-testid="catalogue-search-input"
        class="flex h-11 w-full rounded-xl border border-border/70 bg-card/70 px-4 text-sm outline-none"
        placeholder="Search RAWG..."
        @keydown.enter="void loadShowcase(state.query, 'search')"
      />
      <button
        type="button"
        data-testid="catalogue-search-button"
        class="inline-flex h-11 items-center justify-center gap-2 rounded-xl bg-primary px-4 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white"
        :disabled="state.searching"
        @click="void loadShowcase(state.query, 'search')"
      >
        <Loader2 v-if="state.searching" class="h-4 w-4 animate-spin" />
        <ExternalLink v-else class="h-4 w-4" />
        Search
      </button>
    </div>

    <div v-if="state.error" class="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
      {{ state.error }}
    </div>

    <div v-if="state.loading" class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      <div
        v-for="index in 6"
        :key="index"
        class="aspect-[16/9] animate-pulse rounded-2xl border border-border/70 bg-card/60"
      />
    </div>

    <div v-else class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      <article
        v-for="item in state.items"
        :key="item.id"
        :data-testid="`catalogue-card-${item.id}`"
        class="overflow-hidden rounded-2xl border border-border/70 bg-card/80 shadow-[0_18px_40px_rgba(0,0,0,0.22)]"
      >
        <div class="relative aspect-[16/9] overflow-hidden bg-black/30">
          <img
            v-if="item.background_image"
            :src="item.background_image"
            :alt="item.name"
            class="h-full w-full object-cover"
          />
          <div v-else class="flex h-full w-full items-center justify-center text-muted-foreground">
            <Gamepad2 class="h-10 w-10" />
          </div>
          <div class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/80 to-transparent p-4">
            <div class="flex items-center justify-between gap-3">
              <div class="min-w-0">
                <div class="truncate text-base font-semibold text-white">{{ item.name }}</div>
                <div class="mt-1 truncate text-xs text-white/70">
                  {{ translateGenreListToRu(item.genres?.map((genre) => genre.name).join(", ") ?? null, 2).join(" · ") }}
                </div>
              </div>
              <div
                v-if="item.rating"
                class="inline-flex items-center gap-1 rounded-full border border-white/15 bg-black/25 px-2.5 py-1 text-xs text-white"
              >
                <Star class="h-3.5 w-3.5" />
                {{ item.rating.toFixed(1) }}
              </div>
            </div>
          </div>
        </div>

        <div class="space-y-4 p-4">
          <div class="text-sm text-muted-foreground">
            Released: {{ item.released ?? "Unknown" }}
          </div>

          <div class="flex flex-wrap items-center gap-2">
            <RouterLink
              v-if="getLibraryGameLink(item)"
              :to="getLibraryGameLink(item) ?? '/catalogue'"
              class="inline-flex h-10 items-center justify-center rounded-xl border border-border/70 bg-card/80 px-4 text-sm font-[510] transition-colors hover:bg-accent/70"
            >
              Open in library
            </RouterLink>
            <button
              v-else
              type="button"
              :data-testid="`catalogue-add-${item.id}`"
              class="inline-flex h-10 items-center justify-center gap-2 rounded-xl bg-primary px-4 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white"
              :disabled="state.addingId === item.id"
              @click="void addToLibrary(item)"
            >
              <Loader2 v-if="state.addingId === item.id" class="h-4 w-4 animate-spin" />
              <Plus v-else class="h-4 w-4" />
              Add to library
            </button>
          </div>
        </div>
      </article>
    </div>
  </div>
</template>
