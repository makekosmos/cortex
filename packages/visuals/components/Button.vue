<script setup lang="ts">
// Button — общий primary/ghost/danger primitive поверх @kosmos/visuals tokens.
// Заменяет ad-hoc `.btn` / `.btn.ghost` CSS, тиражирующиеся в SettingsView и
// прочих местах. v-slot для текста + опциональный leading icon через
// именованный slot `icon`.

withDefaults(
  defineProps<{
    /** Внешний вид. */
    variant?: "primary" | "ghost" | "danger";
    /** Размер: md (default, 34px h) или sm (28px h). */
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
      'kosmos-btn',
      `kosmos-btn--${variant}`,
      `kosmos-btn--${size}`,
      { 'kosmos-btn--block': block, 'kosmos-btn--loading': loading },
    ]"
  >
    <span v-if="$slots.icon" class="kosmos-btn__icon"><slot name="icon" /></span>
    <span class="kosmos-btn__label"><slot /></span>
  </button>
</template>

<style scoped>
.kosmos-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  height: 34px;
  padding: 0 0.9rem;
  border: 2px solid transparent;
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  font-family: inherit;
  font-size: 0.875rem;
  font-weight: 500;
  line-height: 1;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    border-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-btn--sm {
  height: 28px;
  padding: 0 0.7rem;
  font-size: 0.8125rem;
}

.kosmos-btn--block {
  width: 100%;
}

.kosmos-btn--primary {
  background: var(--accent);
  color: var(--accent-foreground, var(--background));
  border-color: var(--accent);
}

.kosmos-btn--primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--accent) 88%, white);
  border-color: color-mix(in srgb, var(--accent) 88%, white);
}

.kosmos-btn--primary:active:not(:disabled) {
  background: color-mix(in srgb, var(--accent) 80%, black);
}

.kosmos-btn--ghost {
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  color: var(--foreground);
  border-color: var(--border);
}

.kosmos-btn--ghost:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  background: color-mix(in srgb, var(--foreground) 7%, var(--background));
}

.kosmos-btn--danger {
  background: transparent;
  color: var(--destructive);
  border-color: color-mix(in srgb, var(--destructive) 35%, var(--border));
}

.kosmos-btn--danger:hover:not(:disabled) {
  background: color-mix(in srgb, var(--destructive) 10%, transparent);
  border-color: var(--destructive);
}

.kosmos-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.kosmos-btn--loading .kosmos-btn__label {
  opacity: 0.7;
}

.kosmos-btn__icon {
  display: inline-flex;
  align-items: center;
}
</style>
