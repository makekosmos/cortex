<script setup lang="ts">
import { Clock, Search, X } from "lucide-vue-next";
import {
  computed,
  nextTick,
  onMounted,
  onUnmounted,
  shallowRef,
  watch,
} from "vue";
import { useRoute, useRouter } from "vue-router";
import type { Game } from "../../src/types";
import { useLanguage } from "../composables/useLanguage";
import { useGamesStore } from "../stores/games";

const MAX_RESULTS = 12;

const props = withDefaults(
  defineProps<{
    triggerClassName?: string;
    showTrigger?: boolean;
    triggerVariant?: "default" | "sidebar";
    enableShortcut?: boolean;
  }>(),
  {
    triggerClassName: undefined,
    showTrigger: true,
    triggerVariant: "default",
    enableShortcut: true,
  },
);

const route = useRoute();
const router = useRouter();
const gamesStore = useGamesStore();
const { language } = useLanguage();

const open = shallowRef(false);
const searchQuery = shallowRef("");
const activeIndex = shallowRef(0);
const inputRef = shallowRef<HTMLInputElement | null>(null);
const panelRef = shallowRef<HTMLDivElement | null>(null);

function formatPlaytime(seconds: number) {
  if (!seconds) {
    return language.value === "ru" ? "0 ч" : "0 h";
  }

  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (hours > 0) {
    return language.value === "ru"
      ? `${hours} ч ${minutes} мин`
      : `${hours} h ${minutes} m`;
  }

  return language.value === "ru" ? `${minutes} мин` : `${minutes} m`;
}

function makeMatchScore(game: Game, query: string) {
  const normalized = query.toLowerCase();
  const name = game.name.toLowerCase();
  const exeName = game.exe_name.toLowerCase();

  if (name === normalized || exeName === normalized) return 0;
  if (name.startsWith(normalized) || exeName.startsWith(normalized)) return 1;
  if (name.includes(normalized) || exeName.includes(normalized)) return 2;
  return Number.POSITIVE_INFINITY;
}

const labels = computed(() =>
  language.value === "ru"
    ? {
        button: "Поиск",
        placeholder: "Поиск игр...",
        close: "Закрыть",
        title: "Быстрый поиск",
        loading: "Загрузка библиотеки...",
        noQuery: "В библиотеке пока нет игр",
        noMatch: "Нет подходящих игр",
      }
    : {
        button: "Search",
        placeholder: "Search games...",
        close: "Close",
        title: "Quick search",
        loading: "Loading library...",
        noQuery: "No games in library yet",
        noMatch: "No matching games",
      },
);

const filteredGames = computed(() => {
  if (gamesStore.loading) {
    return [] as Game[];
  }

  const query = searchQuery.value.trim().toLowerCase();

  if (!query) {
    return [...gamesStore.games]
      .sort((left, right) => {
        if (left.is_favorite !== right.is_favorite) {
          return left.is_favorite ? -1 : 1;
        }

        return left.name.localeCompare(right.name);
      })
      .slice(0, MAX_RESULTS);
  }

  return gamesStore.games
    .map((game) => ({
      game,
      score: makeMatchScore(game, query),
    }))
    .filter((entry) => Number.isFinite(entry.score))
    .sort((left, right) => {
      if (left.score !== right.score) return left.score - right.score;
      if (left.game.is_favorite !== right.game.is_favorite) {
        return left.game.is_favorite ? -1 : 1;
      }

      return left.game.name.localeCompare(right.game.name);
    })
    .slice(0, MAX_RESULTS)
    .map((entry) => entry.game);
});

function close() {
  open.value = false;
  searchQuery.value = "";
  activeIndex.value = 0;
}

async function openPanel() {
  searchQuery.value = "";
  activeIndex.value = 0;
  open.value = true;
  await nextTick();
  inputRef.value?.focus();
}

async function selectGame(game: Game) {
  close();
  await router.push(`/game/${game.id}`);
}

function getFocusableElements() {
  return Array.from(
    panelRef.value?.querySelectorAll<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])',
    ) ?? [],
  );
}

watch(
  () => route.fullPath,
  () => {
    if (open.value) {
      close();
    }
  },
);

watch(filteredGames, (games) => {
  if (games.length <= activeIndex.value) {
    activeIndex.value = 0;
  }
});

watch([open, activeIndex], async () => {
  if (!open.value || filteredGames.value.length === 0) {
    return;
  }

  await nextTick();
  const option = panelRef.value?.querySelector<HTMLElement>(
    `[data-spotlight-option-index="${activeIndex.value}"]`,
  );
  option?.scrollIntoView({ block: "nearest" });
});

function onKeyDown(event: KeyboardEvent) {
  if (
    props.enableShortcut &&
    (event.ctrlKey || event.metaKey) &&
    event.code === "KeyK"
  ) {
    event.preventDefault();
    if (open.value) {
      close();
    } else {
      void openPanel();
    }
    return;
  }

  if (!open.value) {
    return;
  }

  if (event.key === "Escape") {
    event.preventDefault();
    close();
    return;
  }

  if (event.key === "Tab") {
    const focusables = getFocusableElements();
    if (focusables.length === 0) return;

    const currentIndex = focusables.indexOf(
      document.activeElement as HTMLElement,
    );
    const nextIndex = event.shiftKey
      ? currentIndex <= 0
        ? focusables.length - 1
        : currentIndex - 1
      : currentIndex === focusables.length - 1
        ? 0
        : currentIndex + 1;

    event.preventDefault();
    focusables[nextIndex]?.focus();
    return;
  }

  if (event.key === "ArrowDown") {
    event.preventDefault();
    activeIndex.value =
      filteredGames.value.length === 0
        ? 0
        : Math.min(filteredGames.value.length - 1, activeIndex.value + 1);
    return;
  }

  if (event.key === "ArrowUp") {
    event.preventDefault();
    activeIndex.value = Math.max(0, activeIndex.value - 1);
    return;
  }

  if (event.key === "Enter" && filteredGames.value[activeIndex.value]) {
    event.preventDefault();
    void selectGame(filteredGames.value[activeIndex.value]);
  }
}

onMounted(() => {
  document.addEventListener("keydown", onKeyDown);
});

onUnmounted(() => {
  document.removeEventListener("keydown", onKeyDown);
});
</script>

<template>
  <button
    v-if="!open && showTrigger"
    type="button"
    :aria-label="labels.button"
    :aria-expanded="open"
    :class="[
      triggerVariant === 'sidebar'
        ? 'kepler-sidebar-btn'
        : 'inline-flex h-9 items-center gap-2 rounded-md border border-border/70 bg-card/70 px-3 text-xs font-[510] text-muted-foreground shadow-[0_1px_0_rgba(255,255,255,0.02)] backdrop-blur-md transition-colors hover:border-border hover:bg-card hover:text-foreground',
      triggerClassName,
    ]"
    @click="void openPanel()"
  >
    <Search class="h-4 w-4" />
    <span :class="triggerVariant === 'sidebar' ? 'inline truncate' : 'hidden sm:inline'">
      {{ labels.button }}
    </span>
  </button>

  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-[90] flex items-start justify-center bg-black/84 px-4 pt-16 backdrop-blur-sm"
      @mousedown.self="close"
    >
      <div
        ref="panelRef"
        class="w-full max-w-2xl overflow-hidden rounded-2xl border border-border/70 bg-card/96 shadow-[0_30px_80px_rgba(0,0,0,0.45)] backdrop-blur-xl"
      >
        <div class="flex items-center gap-2 border-b border-border/70 px-3 py-2.5">
          <Search class="h-4 w-4 text-muted-foreground" />
          <input
            ref="inputRef"
            v-model="searchQuery"
            :placeholder="labels.placeholder"
            :aria-label="labels.placeholder"
            class="w-full bg-transparent py-2 text-sm text-foreground outline-none placeholder:text-muted-foreground"
          />
          <button
            type="button"
            class="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent/80 hover:text-foreground"
            :aria-label="labels.close"
            @click="close"
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <div class="max-h-[60vh] overflow-y-auto p-2">
          <div
            v-if="gamesStore.loading"
            class="px-3 py-8 text-center text-sm text-muted-foreground"
          >
            {{ labels.loading }}
          </div>
          <div
            v-else-if="filteredGames.length === 0"
            class="px-3 py-8 text-center text-sm text-muted-foreground"
          >
            {{ searchQuery.trim() ? labels.noMatch : labels.noQuery }}
          </div>
          <ul v-else class="space-y-1" role="listbox" :aria-label="labels.title">
            <li v-for="(game, index) in filteredGames" :key="game.id">
              <button
                type="button"
                role="option"
                :aria-selected="index === activeIndex"
                :data-spotlight-option-index="index"
                class="group flex w-full items-center gap-3 rounded-xl border px-2 py-2 text-left transition-colors"
                :class="
                  index === activeIndex
                    ? 'border-border/80 bg-accent/80 text-accent-foreground shadow-[0_1px_0_rgba(255,255,255,0.02)]'
                    : 'border-transparent text-foreground/90 hover:border-border/70 hover:bg-accent/60'
                "
                @mouseenter="activeIndex = index"
                @click="void selectGame(game)"
              >
                <div class="flex h-10 w-10 flex-shrink-0 overflow-hidden rounded-lg bg-muted">
                  <img
                    v-if="game.background_image"
                    :src="game.background_image"
                    :alt="game.name"
                    class="h-full w-full object-cover"
                    loading="lazy"
                    decoding="async"
                  />
                </div>

                <div class="min-w-0 flex-1">
                  <div class="truncate font-[510]">{{ game.name }}</div>
                  <div class="truncate text-xs text-muted-foreground">
                    {{ game.exe_name }}
                  </div>
                </div>

                <div
                  v-if="game.total_playtime > 0"
                  class="hidden items-center gap-1 text-xs text-muted-foreground sm:flex"
                >
                  <Clock class="h-3 w-3" />
                  {{ formatPlaytime(game.total_playtime) }}
                </div>
              </button>
            </li>
          </ul>
        </div>
      </div>
    </div>
  </Teleport>
</template>
