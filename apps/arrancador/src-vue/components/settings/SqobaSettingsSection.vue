<script setup lang="ts">
import { Loader2, RefreshCw, Sparkles } from "lucide-vue-next";
import { RouterLink } from "vue-router";
import type { InlineFeedback } from "./types";

const props = withDefaults(
  defineProps<{
    sectionId?: string;
    manifestRefreshing: boolean;
    manifestFeedback: InlineFeedback | null;
  }>(),
  {
    sectionId: undefined,
  },
);

const emit = defineEmits<{
  refreshManifest: [];
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
      <Sparkles class="h-5 w-5" />
      <h2 class="text-base font-semibold sm:text-lg">SQOBA</h2>
    </div>

    <div class="space-y-4 rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
      <div class="flex items-center justify-between gap-3 rounded-xl bg-background/40 px-3 py-3">
        <div>
          <div class="text-sm font-medium">Менеджер SQOBA</div>
          <div class="text-xs text-muted-foreground">
            Откройте страницу SQOBA для детальной работы с резервными копиями и путями сохранений.
          </div>
        </div>
        <RouterLink
          to="/sqoba"
          class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70"
        >
          <Sparkles class="h-4 w-4" />
          Открыть SQOBA
        </RouterLink>
      </div>

      <div class="rounded-xl border border-border/70 p-3">
        <div class="flex items-center justify-between gap-3">
          <div>
            <div class="text-sm font-medium">Обновление манифеста</div>
            <div class="text-xs text-muted-foreground">
              Подтянуть актуальный локальный список игр и соответствий путей сохранений.
            </div>
          </div>
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-border/70 bg-card/80 px-3 py-2 text-sm transition-colors hover:bg-accent/70 disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="props.manifestRefreshing"
            @click="emit('refreshManifest')"
          >
            <Loader2 v-if="props.manifestRefreshing" class="h-3.5 w-3.5 animate-spin" />
            <RefreshCw v-else class="h-3.5 w-3.5" />
            Обновить
          </button>
        </div>
        <div
          v-if="props.manifestFeedback"
          class="mt-2 text-xs"
          :class="feedbackClass(props.manifestFeedback)"
        >
          {{ props.manifestFeedback.text }}
        </div>
      </div>
    </div>
  </section>
</template>
