<script setup lang="ts">
import { X } from "lucide-vue-next";
import { cn } from "../../src/lib/utils";
import { useToast } from "../composables/useToast";

const { toasts, removeToast } = useToast();

const toneStyles = {
  info: "border-border/70 bg-card/90",
  success: "border-emerald-500/25 bg-emerald-500/8",
  warning: "border-amber-400/25 bg-amber-400/8",
  error: "border-red-500/25 bg-red-500/8",
} as const;

const toneAccent = {
  info: "text-foreground",
  success: "text-emerald-400",
  warning: "text-amber-300",
  error: "text-red-400",
} as const;
</script>

<template>
  <div class="fixed bottom-4 right-4 z-50 flex w-full max-w-sm flex-col gap-2 px-4 lg:px-0">
    <div
      v-for="toast in toasts"
      :key="toast.id"
      :role="toast.tone === 'error' ? 'alert' : 'status'"
      :aria-live="toast.tone === 'error' ? 'assertive' : 'polite'"
      aria-atomic="true"
      :class="
        cn(
          'pointer-events-auto rounded-xl border bg-card/90 px-4 py-3 shadow-[0_18px_40px_rgba(0,0,0,0.32)] backdrop-blur-xl',
          toneStyles[toast.tone ?? 'info'],
        )
      "
    >
      <div class="flex items-start gap-3">
        <div class="flex-1">
          <div :class="cn('text-sm font-[510]', toneAccent[toast.tone ?? 'info'])">
            {{ toast.title }}
          </div>
          <div
            v-if="toast.description"
            class="mt-1 text-xs leading-relaxed text-muted-foreground"
          >
            {{ toast.description }}
          </div>
        </div>
        <button
          type="button"
          class="text-muted-foreground transition-colors hover:text-foreground"
          aria-label="Закрыть уведомление"
          @click="removeToast(toast.id)"
        >
          <X class="h-4 w-4" />
        </button>
      </div>
    </div>
  </div>
</template>
