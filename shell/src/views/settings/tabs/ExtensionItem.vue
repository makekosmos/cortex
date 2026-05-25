<script setup lang="ts">
// ExtensionItem — единая card для строки extension'а в ExtensionsTab.
// Используется и для marketplace-catalog (ещё не установленных), и для
// installed extension'ов. Разница только в данных и action-кнопках; layout
// (icon + ext-info + ext-actions) одинаковый. Actions передаются через
// `actions` slot.

defineProps<{
  /** Главная иконка: data-uri (для installed) или URL (для marketplace).
   * Если null — рендерим fallback с первой буквой `name`. */
  iconSrc?: string | null;
  name: string;
  /** "v1.2.3" или "v—". */
  version: string;
  author?: string | null;
  description?: string | null;
}>();
</script>

<template>
  <div class="ext-item">
    <img
      v-if="iconSrc"
      class="ext-icon"
      :src="iconSrc"
      alt=""
      @error="(e) => ((e.target as HTMLImageElement).style.display = 'none')"
    />
    <div v-else class="ext-icon ext-icon-fallback">
      {{ name.slice(0, 1) }}
    </div>
    <div class="ext-info">
      <div class="ext-name">{{ name }}</div>
      <div class="ext-meta">
        <span class="ext-version">{{ version }}</span>
        <slot name="meta" />
        <span v-if="author" class="ext-author">· {{ author }}</span>
      </div>
      <div v-if="description" class="ext-description">
        {{ description }}
      </div>
    </div>
    <div class="ext-actions">
      <slot name="actions" />
    </div>
  </div>
</template>
