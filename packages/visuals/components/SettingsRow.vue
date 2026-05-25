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
  gap: 16px;
  padding: 14px 12px;
  /* Без своего background — `--settings-list-background` стоит на родителе
   * (SettingsList). С полупрозрачным overlay двойной слой здесь стекался и
   * прямоугольник в Security выглядел темнее остальных. */
  background: transparent;
  border-bottom: 1px solid var(--border-color-strong);
}

.kosmos-settings-row:last-child {
  border-bottom: none;
}

.kosmos-settings-row__left {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.kosmos-settings-row__title {
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 1.4;
  font-weight: 500;
  color: var(--foreground);
}

.kosmos-settings-row__description {
  font-family: var(--font-sans);
  font-size: 11px;
  line-height: 1.4;
  font-weight: 500;
  color: var(--muted-foreground);
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
