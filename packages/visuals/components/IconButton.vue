<script setup lang="ts">
// IconButton — ghost icon button primitive с @kosmos/visuals tokens.
// Используется в titlebar'ах (Horologion App.vue), in mini-player widget'е,
// в context-aware controls. Заменяет ad-hoc `.iconbtn` / `.ctl-btn` /
// `.close-btn` CSS, которые тиражировались по экосистеме.

withDefaults(
  defineProps<{
    /** Side length в px. Default 28 (titlebar standard), 24 для mini-player. */
    size?: number;
    /** Border-radius в px. Default — половина size для пилюли. */
    radius?: number;
    /** Tone влияет на hover-цвет:
        - default — нейтральный foreground hover;
        - destructive — destructive token на hover (для удаления / закрытия). */
    tone?: "default" | "destructive";
    /** Если true — оставляет область draggable (для `-webkit-app-region: drag`
        контекстов). По умолчанию false → область кнопки явно no-drag, иначе
        её нельзя кликнуть в drag-area mini-player'а. */
    draggable?: boolean;
    /** Button type — по умолчанию "button" (не submit). */
    type?: "button" | "submit";
    /** Disabled state. */
    disabled?: boolean;
  }>(),
  { size: 28, tone: "default", draggable: false, type: "button", disabled: false },
);
</script>

<template>
  <button
    :type="type"
    :disabled="disabled"
    :class="['icon-btn', `icon-btn--${tone}`, { 'icon-btn--no-drag': !draggable }]"
    :style="{
      '--icon-btn-size': `${size}px`,
      '--icon-btn-radius': `${radius ?? Math.min(8, size / 3)}px`,
    }"
  >
    <slot />
  </button>
</template>

<style scoped>
.icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--icon-btn-size);
  height: var(--icon-btn-size);
  padding: 0;
  background: transparent;
  border: none;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  border-radius: var(--icon-btn-radius);
  cursor: pointer;
  flex-shrink: 0;
  transition:
    color 120ms cubic-bezier(0.2, 0, 0, 1),
    background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.icon-btn--no-drag {
  -webkit-app-region: no-drag;
}

.icon-btn:hover {
  color: var(--foreground);
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.icon-btn:active {
  background: color-mix(in srgb, var(--foreground) 14%, transparent);
}

.icon-btn--destructive:hover {
  color: var(--destructive);
  background: color-mix(in srgb, var(--destructive) 12%, transparent);
}

.icon-btn:disabled {
  cursor: not-allowed;
  opacity: 0.4;
  pointer-events: none;
}
</style>
