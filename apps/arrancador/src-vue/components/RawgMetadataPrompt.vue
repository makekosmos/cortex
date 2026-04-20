<script setup lang="ts">
import { computed, onMounted, onUnmounted, shallowRef, watch } from "vue";
import { ExternalLink, Gamepad2, Loader2, Search, X } from "lucide-vue-next";
import { RouterLink } from "vue-router";
import { metadataApi } from "../../src/lib/api";
import type { Game, RawgGame } from "../../src/types";
import { useToast } from "../composables/useToast";

type GameLite = Pick<Game, "id" | "name">;

const props = defineProps<{
  game: GameLite;
  remaining: number;
}>();

const emit = defineEmits<{
  next: [];
  skipAll: [];
  afterApply: [];
}>();

const { notify } = useToast();

const query = shallowRef(props.game.name);
const results = shallowRef<RawgGame[]>([]);
const searching = shallowRef(false);
const applyingId = shallowRef<number | null>(null);
const renameFromMetadata = shallowRef(false);
const rawgKeyStatus = shallowRef<"unknown" | "present" | "missing">("unknown");
const panelRef = shallowRef<HTMLDivElement | null>(null);
const inputRef = shallowRef<HTMLInputElement | null>(null);

let searchSeq = 0;
let applySeq = 0;

const hasApiKey = computed(() => rawgKeyStatus.value === "present");

watch(
  () => props.game,
  async (game) => {
    query.value = game.name;
    results.value = [];
    searching.value = false;
    applyingId.value = null;
    rawgKeyStatus.value = "unknown";

    try {
      const key = await metadataApi.getApiKey();
      rawgKeyStatus.value = key?.trim() ? "present" : "missing";
    } catch {
      rawgKeyStatus.value = "missing";
    }
  },
  { immediate: true, deep: true },
);

watch(
  () => props.game.id,
  () => {
    if (hasApiKey.value && query.value.trim()) {
      void searchMetadata(query.value);
    }
  },
);

function getFocusableElements() {
  return Array.from(
    panelRef.value?.querySelectorAll<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])',
    ) ?? [],
  );
}

function onKeyDown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    emit("next");
    return;
  }

  if (event.key !== "Tab") {
    return;
  }

  const focusables = getFocusableElements();
  if (focusables.length === 0) return;

  const currentIndex = focusables.indexOf(document.activeElement as HTMLElement);
  const nextIndex = event.shiftKey
    ? currentIndex <= 0
      ? focusables.length - 1
      : currentIndex - 1
    : currentIndex === focusables.length - 1
      ? 0
      : currentIndex + 1;

  event.preventDefault();
  focusables[nextIndex]?.focus();
}

async function searchMetadata(forcedQuery?: string) {
  const nextQuery = (forcedQuery ?? query.value).trim();
  if (!nextQuery) return;

  const seq = ++searchSeq;
  searching.value = true;

  try {
    const found = await metadataApi.search(nextQuery);
    if (searchSeq !== seq) return;
    results.value = found;

    if (found.length === 0) {
      notify({
        tone: "info",
        title: "Ничего не найдено",
        description: "Попробуйте уточнить запрос.",
        durationMs: 2600,
      });
    }
  } catch (cause) {
    if (searchSeq !== seq) return;
    console.error("RAWG search failed:", cause);
    notify({
      tone: "error",
      title: "Не удалось найти метаданные",
      description: "Проверьте RAWG API ключ в настройках.",
    });
  } finally {
    if (searchSeq === seq) {
      searching.value = false;
    }
  }
}

async function applyMetadata(rawgGame: RawgGame) {
  const seq = ++applySeq;
  applyingId.value = rawgGame.id;

  try {
    await metadataApi.apply(props.game.id, rawgGame.id, renameFromMetadata.value);
    if (applySeq !== seq) return;

    emit("afterApply");
    notify({
      tone: "success",
      title: "Метаданные применены",
      description: rawgGame.name,
      durationMs: 2400,
    });
    emit("next");
  } catch (cause) {
    if (applySeq !== seq) return;
    console.error("Failed to apply RAWG metadata:", cause);
    notify({
      tone: "error",
      title: "Не удалось применить метаданные",
    });
  } finally {
    if (applySeq === seq) {
      applyingId.value = null;
    }
  }
}

onMounted(() => {
  document.addEventListener("keydown", onKeyDown);
  window.setTimeout(() => inputRef.value?.focus(), 0);
});

onUnmounted(() => {
  document.removeEventListener("keydown", onKeyDown);
});
</script>

<template>
  <Teleport to="body">
    <div
      role="dialog"
      aria-modal="true"
      class="fixed inset-0 z-[120] flex items-center justify-center bg-black/75 p-4 backdrop-blur-sm"
      @mousedown.self="emit('next')"
    >
      <div
        ref="panelRef"
        class="relative flex max-h-[85vh] w-full max-w-xl flex-col overflow-hidden rounded-2xl border border-border/70 bg-card shadow-[0_30px_80px_rgba(0,0,0,0.45)]"
      >
        <div class="pointer-events-none absolute inset-0">
          <div
            class="absolute -top-24 -right-24 h-56 w-56 rounded-full blur-3xl opacity-60"
            style="background: radial-gradient(circle, rgba(56,189,248,0.22), rgba(56,189,248,0));"
          />
          <div
            class="absolute -bottom-24 -left-24 h-56 w-56 rounded-full blur-3xl opacity-50"
            style="background: radial-gradient(circle, rgba(244,63,94,0.18), rgba(244,63,94,0));"
          />
        </div>

        <div class="relative border-b border-border/70 p-4">
          <div class="flex items-start gap-3">
            <div class="min-w-0 flex-1">
              <div class="text-sm font-semibold">Добавить метаданные из RAWG</div>
              <div class="mt-0.5 truncate text-xs text-muted-foreground">
                {{ game.name }}
                <span v-if="remaining > 1"> · Осталось: {{ remaining }}</span>
              </div>
            </div>
            <button
              type="button"
              class="text-muted-foreground transition-colors hover:text-foreground"
              aria-label="Закрыть"
              @click="emit('skipAll')"
            >
              <X class="h-4 w-4" />
            </button>
          </div>
        </div>

        <div class="relative flex flex-1 flex-col overflow-hidden p-4">
          <div
            v-if="rawgKeyStatus === 'missing'"
            class="mb-4 rounded-xl border border-border/70 bg-secondary/20 p-3"
          >
            <div class="text-sm font-semibold">Нужен RAWG API ключ</div>
            <div class="mt-1 text-xs leading-relaxed text-muted-foreground">
              Добавьте ключ в настройках, чтобы искать метаданные.
            </div>
            <div class="mt-3 flex items-center justify-end gap-2">
              <button
                type="button"
                class="inline-flex h-9 items-center rounded-xl px-3 text-sm text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
                @click="emit('next')"
              >
                Не сейчас
              </button>
              <RouterLink
                to="/settings"
                class="inline-flex h-9 items-center rounded-xl bg-primary px-3 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white"
              >
                Открыть настройки
              </RouterLink>
            </div>
          </div>

          <div class="mb-4 flex gap-2">
            <input
              ref="inputRef"
              v-model="query"
              placeholder="Название игры..."
              aria-label="Название игры"
              class="h-10 flex-1 rounded-xl border border-border/70 bg-background/60 px-3 text-sm outline-none"
              :disabled="!hasApiKey"
              @keydown.enter="void searchMetadata()"
            />
            <button
              type="button"
              class="inline-flex h-10 w-10 items-center justify-center rounded-xl bg-primary text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="!hasApiKey || searching"
              @click="void searchMetadata()"
            >
              <Loader2 v-if="searching" class="h-4 w-4 animate-spin" />
              <Search v-else class="h-4 w-4" />
            </button>
          </div>

          <div class="mb-3 flex items-center justify-between gap-3 text-sm text-muted-foreground">
            <span id="rawg-rename-toggle">Использовать название из RAWG</span>
            <button
              type="button"
              class="inline-flex h-6 w-11 items-center rounded-full border border-border/70 px-1 transition-colors"
              :class="renameFromMetadata ? 'bg-primary/20' : 'bg-card/60'"
              aria-labelledby="rawg-rename-toggle"
              @click="renameFromMetadata = !renameFromMetadata"
            >
              <span
                class="h-4 w-4 rounded-full bg-white transition-transform"
                :class="renameFromMetadata ? 'translate-x-5' : 'translate-x-0'"
              />
            </button>
          </div>

          <div class="flex-1 overflow-y-auto">
            <div v-if="results.length > 0" class="space-y-2">
              <button
                v-for="result in results"
                :key="result.id"
                type="button"
                class="flex w-full items-center gap-3 rounded-xl border border-transparent p-3 text-left transition-colors hover:border-border hover:bg-secondary/70 disabled:cursor-not-allowed disabled:opacity-70"
                :disabled="applyingId !== null"
                @click="void applyMetadata(result)"
              >
                <img
                  v-if="result.background_image"
                  :src="result.background_image"
                  :alt="result.name"
                  class="h-14 w-14 rounded-lg border border-border/60 object-cover"
                  loading="lazy"
                  decoding="async"
                />
                <div
                  v-else
                  class="flex h-14 w-14 items-center justify-center rounded-lg border border-border/60 bg-muted"
                >
                  <Gamepad2 class="h-6 w-6 text-muted-foreground" />
                </div>

                <div class="min-w-0 flex-1">
                  <div class="truncate font-medium">{{ result.name }}</div>
                  <div class="mt-0.5 text-xs text-muted-foreground">
                    {{ result.released?.slice(0, 4) ?? "—" }}
                    <span v-if="result.metacritic != null"> · MC {{ result.metacritic }}</span>
                  </div>
                </div>

                <Loader2
                  v-if="applyingId === result.id"
                  class="h-4 w-4 animate-spin text-primary"
                />
                <ExternalLink v-else class="h-4 w-4 text-muted-foreground" />
              </button>
            </div>
            <div v-else class="py-10 text-center text-sm text-muted-foreground">
              {{
                hasApiKey
                  ? "Введите название и нажмите поиск"
                  : "Добавьте RAWG API ключ в настройках"
              }}
            </div>
          </div>
        </div>

        <div class="relative flex items-center justify-between gap-2 border-t border-border/70 p-4">
          <button
            type="button"
            class="inline-flex h-9 items-center rounded-xl px-3 text-sm text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
            @click="emit('skipAll')"
          >
            Пропустить все
          </button>
          <button
            type="button"
            class="inline-flex h-9 items-center rounded-xl px-3 text-sm text-muted-foreground transition-colors hover:bg-accent/70 hover:text-foreground"
            @click="emit('next')"
          >
            Пропустить
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
