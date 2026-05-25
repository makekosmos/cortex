<script setup lang="ts">
// LegacyToggle — внутренний shell-локальный toggle, отличается от
// `@kosmos/visuals/Toggle` стилизацией (`.track`/`.thumb` с фиксированными
// размерами 36×20, border-radius 12px, не 999px). Используется во всех
// «legacy»-tab'ах SettingsView (General/Debug/AppCommands/FileSearch), где
// визуальный стиль toggle отличается от kosmos-toggle.
//
// Замещает 8× повторяющийся inline-паттерн в SettingsView.vue:
// `<label class="toggle" :class="{disabled:...}"><input type="checkbox" ...>
// <span class="track"><span class="thumb"/></span></label>`.
//
// CSS правил .toggle/.track/.thumb из родительского `<style scoped>`
// SettingsView переехали сюда 1:1 (в scoped-блок этого компонента).

defineProps<{
  checked: boolean;
  disabled?: boolean;
}>();

defineEmits<{ change: [e: Event] }>();
</script>

<template>
  <label class="toggle" :class="{ disabled: disabled }">
    <input
      type="checkbox"
      :checked="checked"
      :disabled="disabled"
      @change="(e: Event) => $emit('change', e)"
    />
    <span class="track"><span class="thumb" /></span>
  </label>
</template>

<style scoped>
.toggle {
  position: relative;
  display: inline-block;
  cursor: pointer;
  flex-shrink: 0;
}

.toggle.disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.toggle input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
  width: 0;
  height: 0;
}

.track {
  display: block;
  width: 36px;
  height: 20px;
  border-radius: 12px;
  background: color-mix(in srgb, var(--foreground) 16%, transparent);
  position: relative;
  transition: background 0.15s ease;
}

.thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--foreground);
  transition: transform 0.15s ease;
}

.toggle input:checked + .track {
  background: var(--accent, oklch(0.7 0.18 250));
}

.toggle input:checked + .track .thumb {
  transform: translateX(16px);
}
</style>
