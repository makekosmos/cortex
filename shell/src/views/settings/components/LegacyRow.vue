<script setup lang="ts">
// LegacyRow — обвязка для legacy `.row + .row-label` паттерна (отличается
// от @kosmos/visuals/SettingsRow — другая CSS-семантика, другие отступы).
// Используется ~25 раз в Settings tab'ах (General/Debug/AppCommands/
// FileSearch/Focus/Export/Extensions). Каждое использование стоило 5-8
// строк inline-HTML — теперь 1-3 строки.
//
// Slot'ы:
// - default: control справа (Toggle, Dropdown, кнопка, code, и т.п.)
// - hint: для случаев когда hint сложный (содержит `<code>`, `<template>`)
// - extra: дополнительный контент под hint (например, ошибки или статусы)
//
// Использует classы `.row`/`.row-label`/`.label`/`.hint` — они живут в
// `settings-shared.css` (namespaced под `.settings-shell`), поэтому работают
// и в дочерних tab-компонентах.

defineProps<{
  /** Заголовок строки. Если нужен сложный label (HTML / inline span hint) —
   *  использовать слот #title. */
  title?: string;
  hint?: string;
  /** Когда есть значение — рендерится как `<div class="error">{{ error }}</div>`
   *  под hint'ом. */
  error?: string;
}>();
</script>

<template>
  <div class="row">
    <div class="row-label">
      <div class="label">
        <slot name="title">{{ title }}</slot>
      </div>
      <div v-if="$slots.hint || hint" class="hint">
        <slot name="hint">{{ hint }}</slot>
      </div>
      <div v-if="error" class="error">{{ error }}</div>
      <slot name="extra" />
    </div>
    <slot />
  </div>
</template>
