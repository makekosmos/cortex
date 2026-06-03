<script setup lang="ts">
// Button — общий primary/ghost/danger primitive поверх @kosmos/visuals tokens.
// Заменяет ad-hoc `.btn` / `.btn.ghost` CSS, тиражирующиеся в SettingsView и
// прочих местах. v-slot для текста + опциональный leading icon через
// именованный slot `icon`.

withDefaults(
  defineProps<{
    /** Внешний вид. */
    variant?: "primary" | "ghost" | "danger";
    /** Размер: md (default, 40px h) или sm (28px h). */
    size?: "md" | "sm";
    /** Кнопка занимает всю ширину контейнера. */
    block?: boolean;
    /** В состоянии загрузки текст приглушён, события не идут. */
    loading?: boolean;
    disabled?: boolean;
    type?: "button" | "submit";
  }>(),
  {
    variant: "primary",
    size: "md",
    block: false,
    loading: false,
    disabled: false,
    type: "button",
  },
);
</script>

<template>
  <button
    :type="type"
    :disabled="disabled || loading"
    :class="[
      'inline-flex cursor-default items-center justify-center gap-2 whitespace-nowrap border-solid border-transparent font-[inherit] font-medium leading-none transition-[background-color,border-color,color] duration-120 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] disabled:cursor-not-allowed disabled:opacity-55',
      size === 'sm'
        ? 'kosmos-btn--sm h-7 rounded-md border px-2.5'
        : 'h-10 rounded-lg border-2 px-4 text-[length:var(--kosmos-text-control-size)]',
      block ? 'w-full' : '',
      variant === 'primary'
        ? 'border-[var(--accent)] bg-[var(--accent)] text-[var(--accent-foreground,var(--background))] hover:not-disabled:border-[color-mix(in_srgb,var(--accent)_88%,white)] hover:not-disabled:bg-[color-mix(in_srgb,var(--accent)_88%,white)] active:not-disabled:bg-[color-mix(in_srgb,var(--accent)_80%,black)]'
        : '',
      variant === 'ghost'
        ? 'border-[var(--border)] bg-[color-mix(in_srgb,var(--foreground)_4%,var(--background))] text-[var(--foreground)] hover:not-disabled:border-[color-mix(in_srgb,var(--accent)_45%,var(--border))] hover:not-disabled:bg-[color-mix(in_srgb,var(--foreground)_7%,var(--background))]'
        : '',
      variant === 'danger'
        ? 'border-[color-mix(in_srgb,var(--destructive)_35%,var(--border))] bg-transparent text-[var(--destructive)] hover:not-disabled:border-[var(--destructive)] hover:not-disabled:bg-[color-mix(in_srgb,var(--destructive)_10%,transparent)]'
        : '',
    ]"
  >
    <span v-if="$slots.icon" class="inline-flex items-center"><slot name="icon" /></span>
    <span :class="loading ? 'opacity-70' : ''"><slot /></span>
  </button>
</template>

<style scoped>
.kosmos-btn--sm {
  font-size: var(--kosmos-text-caption-size);
}
</style>
