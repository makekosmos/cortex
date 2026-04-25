<script setup lang="ts">
import { Database, ExternalLink, FolderOpen, Loader2, RefreshCw } from "lucide-vue-next";
import type { ArkConnectionInfo, GamesArkSyncResult } from "@/types";
import type { InlineFeedback } from "./types";

const props = withDefaults(
  defineProps<{
    sectionId?: string;
    arkConnection: ArkConnectionInfo | null;
    syncPending: boolean;
    syncFeedback: InlineFeedback | null;
    syncResult: GamesArkSyncResult | null;
  }>(),
  {
    sectionId: undefined,
  },
);

const emit = defineEmits<{
  openDatabase: [];
  openDirectory: [];
  syncGames: [];
}>();

function feedbackClass(feedback: InlineFeedback | null) {
  if (!feedback) {
    return "";
  }

  if (feedback.tone === "error") {
    return "text-red-300";
  }

  if (feedback.tone === "success") {
    return "text-emerald-300";
  }

  return "text-muted-foreground";
}
</script>

<template>
  <section :id="props.sectionId" class="space-y-4">
    <div class="flex items-center gap-2">
      <Database class="h-5 w-5" />
      <h2 class="text-base font-semibold sm:text-lg">Ark</h2>
    </div>

    <div class="space-y-4 rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
      <div class="space-y-1">
        <div class="text-sm font-medium">Текущее подключение к Ark</div>
        <p class="text-xs text-muted-foreground">
          Arrancador использует тот же selected space, что и Eden/Delphi. Отдельно
          выбирать Ark-базу внутри Arrancador не нужно.
        </p>
      </div>

      <div class="grid gap-3 sm:grid-cols-2">
        <div class="rounded-xl border border-border/70 bg-background/40 p-3">
          <div class="text-xs uppercase tracking-[0.12em] text-muted-foreground">
            Space code
          </div>
          <div class="mt-1 text-sm font-medium">
            {{ props.arkConnection?.space_code ?? "Локальная Ark DB" }}
          </div>
        </div>

        <div class="rounded-xl border border-border/70 bg-background/40 p-3">
          <div class="text-xs uppercase tracking-[0.12em] text-muted-foreground">
            Статус
          </div>
          <div class="mt-1 text-sm font-medium">
            {{ props.arkConnection?.ark_db_exists ? "База найдена" : "Файл базы не найден" }}
          </div>
        </div>
      </div>

      <div class="rounded-xl border border-border/70 bg-background/40 p-3">
        <div class="text-xs uppercase tracking-[0.12em] text-muted-foreground">
          Путь к Ark DB
        </div>
        <div class="mt-1 break-all text-sm">
          {{ props.arkConnection?.ark_db_path ?? "Путь к Ark DB пока не загружен" }}
        </div>
      </div>

      <div
        v-if="props.arkConnection?.vault_path"
        class="rounded-xl border border-border/70 bg-background/40 p-3"
      >
        <div class="text-xs uppercase tracking-[0.12em] text-muted-foreground">
          Текущий vault
        </div>
        <div class="mt-1 break-all text-sm">
          {{ props.arkConnection.vault_path }}
        </div>
      </div>

      <div class="flex flex-wrap gap-2">
        <button
          type="button"
          class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="!props.arkConnection?.ark_db_path"
          @click="emit('openDatabase')"
        >
          <ExternalLink class="h-4 w-4" />
          Открыть файл базы
        </button>

        <button
          type="button"
          class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="!props.arkConnection?.ark_db_directory"
          @click="emit('openDirectory')"
        >
          <FolderOpen class="h-4 w-4" />
          Открыть папку
        </button>

        <button
          type="button"
          class="inline-flex items-center gap-2 rounded-xl bg-primary px-3 py-2 text-sm font-[510] text-primary-foreground transition-colors hover:bg-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="props.syncPending"
          @click="emit('syncGames')"
        >
          <Loader2 v-if="props.syncPending" class="h-4 w-4 animate-spin" />
          <RefreshCw v-else class="h-4 w-4" />
          Синхронизировать игры в Ark
        </button>
      </div>

      <div
        v-if="props.syncResult"
        class="rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-3 text-sm text-emerald-700 dark:text-emerald-300"
      >
        Синхронизация завершена: {{ props.syncResult.synced }} из {{ props.syncResult.total }}
        <template v-if="props.syncResult.failed > 0">
          , ошибок: {{ props.syncResult.failed }}.
        </template>
        <template v-else>
          , без ошибок.
        </template>
      </div>

      <div
        v-if="props.syncFeedback"
        class="text-sm"
        :class="feedbackClass(props.syncFeedback)"
      >
        {{ props.syncFeedback.text }}
      </div>
    </div>
  </section>
</template>
