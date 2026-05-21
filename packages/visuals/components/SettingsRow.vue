<script setup lang="ts">
// SettingsRow — стандартная строка в Settings UI: title + description слева,
// action / value control справа. Используется в Eden / Horologion / Delphi /
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
  <div class="kosmos-settings-row" :class="{ 'kosmos-settings-row--muted': muted }">
    <div class="kosmos-settings-row__left">
      <div class="kosmos-settings-row__title">{{ title }}</div>
      <div v-if="description" class="kosmos-settings-row__description">
        {{ description }}
      </div>
    </div>
    <div class="kosmos-settings-row__right">
      <slot name="control">
        <slot />
      </slot>
    </div>
  </div>
</template>

<style scoped>
.kosmos-settings-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--border);
}

.kosmos-settings-row:last-child {
  border-bottom: none;
}

.kosmos-settings-row__left {
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
  min-width: 0;
}

.kosmos-settings-row__title {
  font-size: 0.9375rem;
  color: var(--foreground);
}

.kosmos-settings-row__description {
  font-size: 0.8125rem;
  color: var(--muted-foreground);
  line-height: 1.4;
}

.kosmos-settings-row__right {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.kosmos-settings-row--muted .kosmos-settings-row__title,
.kosmos-settings-row--muted .kosmos-settings-row__description {
  opacity: 0.6;
}
</style>
