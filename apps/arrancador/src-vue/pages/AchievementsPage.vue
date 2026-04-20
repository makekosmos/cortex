<script setup lang="ts">
import { computed, onMounted, reactive } from "vue";
import { Award, Gift, Loader2, RefreshCw, Sparkles } from "lucide-vue-next";
import { achievementsApi } from "../../src/lib/api";
import { useToast } from "../composables/useToast";
import type { Achievement } from "../../src/types";

type AchFilter = "all" | "unlocked" | "locked";

const eventTriggers = [
  "game_launch",
  "download_complete",
  "backup_restore",
  "scan_complete",
] as const;

const { notify } = useToast();

const state = reactive<{
  loading: boolean;
  savingDefaults: boolean;
  recording: boolean;
  importing: boolean;
  error: string | null;
  achievements: Achievement[];
  filter: AchFilter;
  eventName: (typeof eventTriggers)[number];
  eventContext: string;
}>({
  loading: false,
  savingDefaults: false,
  recording: false,
  importing: false,
  error: null,
  achievements: [],
  filter: "all",
  eventName: eventTriggers[0],
  eventContext: "",
});

const unlockedCount = computed(() => state.achievements.filter((item) => item.unlocked).length);
const totalCount = computed(() => state.achievements.length);

async function load() {
  state.loading = true;
  state.error = null;

  try {
    const base =
      state.filter === "unlocked"
        ? await achievementsApi.getAll(true)
        : await achievementsApi.getAll(false);

    state.achievements =
      state.filter === "locked" ? base.filter((item) => !item.unlocked) : base;
  } catch (cause) {
    console.error("Failed to load achievements:", cause);
    state.error = "Failed to load achievements";
    notify({ title: "Failed to load achievements", tone: "error" });
  } finally {
    state.loading = false;
  }
}

async function seedDefaults() {
  state.savingDefaults = true;
  try {
    await achievementsApi.seedDefaults();
    await load();
    notify({ title: "Default achievements restored", tone: "success" });
  } catch (cause) {
    console.error("Failed to seed achievements:", cause);
    notify({ title: "Failed to seed achievements", tone: "error" });
  } finally {
    state.savingDefaults = false;
  }
}

async function recordEvent() {
  state.recording = true;
  try {
    await achievementsApi.recordEvent(state.eventName, state.eventContext.trim() || undefined);
    await load();
    notify({ title: "Achievement event recorded", tone: "success" });
  } catch (cause) {
    console.error("Failed to record achievement event:", cause);
    notify({ title: "Failed to record achievement event", tone: "error" });
  } finally {
    state.recording = false;
  }
}

async function exportAchievements() {
  try {
    await navigator.clipboard.writeText(JSON.stringify(state.achievements, null, 2));
    notify({ title: "Achievement export copied", tone: "success" });
  } catch {
    notify({ title: "Failed to export achievements", tone: "error" });
  }
}

async function importAchievements() {
  const raw = window.prompt("Paste achievement JSON to import");
  if (!raw) return;

  state.importing = true;
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      throw new Error("Invalid payload");
    }
    state.achievements = parsed as Achievement[];
    notify({ title: "Imported achievements from clipboard JSON", tone: "success" });
  } catch {
    notify({ title: "Invalid JSON payload", tone: "error" });
  } finally {
    state.importing = false;
  }
}

onMounted(() => {
  void load();
});
</script>

<template>
  <div class="space-y-6 p-6">
    <div class="flex flex-wrap items-start justify-between gap-4">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Достижения</h1>
        <p class="mt-1 text-sm text-muted-foreground">
          {{ unlockedCount }} из {{ totalCount }} разблокировано
        </p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          class="inline-flex h-10 items-center gap-2 rounded-full border border-border/70 bg-card/80 px-4 text-sm font-[510] transition-colors hover:bg-accent/70"
          :disabled="state.savingDefaults"
          @click="seedDefaults"
        >
          <Loader2 v-if="state.savingDefaults" class="h-4 w-4 animate-spin" />
          <RefreshCw v-else class="h-4 w-4" />
          Reset
        </button>
        <button
          type="button"
          class="inline-flex h-10 items-center gap-2 rounded-full border border-border/70 bg-card/80 px-4 text-sm font-[510] transition-colors hover:bg-accent/70"
          :disabled="state.importing"
          @click="importAchievements"
        >
          <Gift class="h-4 w-4" />
          Import
        </button>
        <button
          type="button"
          class="inline-flex h-10 items-center gap-2 rounded-full border border-border/70 bg-card/80 px-4 text-sm font-[510] transition-colors hover:bg-accent/70"
          @click="exportAchievements"
        >
          <Sparkles class="h-4 w-4" />
          Export
        </button>
      </div>
    </div>

    <div class="grid gap-4 lg:grid-cols-[280px_minmax(0,1fr)]">
      <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
        <div class="mb-4 text-sm font-[510]">Фильтр</div>
        <div class="flex gap-2">
          <button
            v-for="option in ['all', 'unlocked', 'locked'] as const"
            :key="option"
            type="button"
            class="inline-flex h-9 items-center rounded-full border px-3 text-sm transition-colors"
            :class="
              state.filter === option
                ? 'border-primary bg-primary text-primary-foreground'
                : 'border-border/70 bg-background/50 text-foreground hover:bg-accent/60'
            "
            @click="
              state.filter = option;
              void load();
            "
          >
            {{ option }}
          </button>
        </div>

        <div class="mt-6 space-y-3">
          <div class="text-sm font-[510]">Записать событие</div>
          <select
            v-model="state.eventName"
            class="flex h-10 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
          >
            <option v-for="eventName in eventTriggers" :key="eventName" :value="eventName">
              {{ eventName }}
            </option>
          </select>
          <input
            v-model="state.eventContext"
            class="flex h-10 w-full rounded-xl border border-border/70 bg-background/50 px-3 text-sm outline-none"
            placeholder="Event context"
          />
          <button
            type="button"
            class="inline-flex h-10 w-full items-center justify-center gap-2 rounded-xl bg-primary px-4 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white"
            :disabled="state.recording"
            @click="recordEvent"
          >
            <Loader2 v-if="state.recording" class="h-4 w-4 animate-spin" />
            <Award v-else class="h-4 w-4" />
            Record
          </button>
        </div>
      </section>

      <section class="rounded-2xl border border-border/70 bg-card/80 p-5 shadow-[0_12px_30px_rgba(0,0,0,0.18)]">
        <div v-if="state.loading" class="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 class="h-4 w-4 animate-spin" />
          Loading achievements...
        </div>
        <div v-else-if="state.error" class="text-sm text-red-300">
          {{ state.error }}
        </div>
        <div v-else class="space-y-3">
          <div
            v-for="item in state.achievements"
            :key="item.id"
            class="rounded-xl border border-border/60 bg-background/40 p-4"
          >
            <div class="flex items-start justify-between gap-4">
              <div>
                <div class="text-sm font-medium">{{ item.title }}</div>
                <div class="mt-1 text-xs leading-5 text-muted-foreground">
                  {{ item.description }}
                </div>
              </div>
              <div
                class="rounded-full border px-2 py-1 text-[11px]"
                :class="
                  item.unlocked
                    ? 'border-emerald-500/35 bg-emerald-500/10 text-emerald-300'
                    : 'border-border/70 bg-card/60 text-muted-foreground'
                "
              >
                {{ item.unlocked ? "Unlocked" : "Locked" }}
              </div>
            </div>
            <div class="mt-3 h-2 overflow-hidden rounded-full border border-border/60 bg-white/6">
              <div
                class="h-full bg-primary transition-all"
                :style="{ width: `${Math.min(100, (item.progress / item.target) * 100)}%` }"
              />
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>
