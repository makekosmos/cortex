<script setup lang="ts">
// SettingsRow — стандартная строка в Settings UI: title + description слева,
// action / value control справа. Используется в Eden / Delphi /
// Arrancador settings.
//
// Slot `control` — место для Toggle, select, button, input, etc.
// Slot `default` — fallback alternative для control.

interface Props {
  title: string;
  description?: string;
  /** Когда true — строка визуально выглядит disabled (но control сам
   * управляет своим disabled-state). */
  muted?: boolean;
}

withDefaults(defineProps<Props>(), {
  muted: false,
});
</script>

<template>
  <div
    class="flex items-start justify-between gap-4 border-b border-[var(--border-color-strong)] bg-transparent px-4 py-4 last:border-b-0"
  >
    <div class="flex min-w-0 flex-col gap-1" :class="{ 'opacity-60': muted }">
      <div
        class="font-[var(--font-sans)] text-[length:var(--kosmos-text-body-size)] leading-[1.4] font-medium text-[var(--foreground)]"
      >
        {{ title }}
      </div>
      <div
        v-if="description"
        class="font-[var(--font-sans)] text-[length:var(--kosmos-text-caption-size)] leading-[1.4] font-medium text-[var(--muted-foreground)]"
      >
        {{ description }}
      </div>
    </div>
    <div class="flex shrink-0 items-start gap-2">
      <slot name="control">
        <slot />
      </slot>
    </div>
  </div>
</template>
