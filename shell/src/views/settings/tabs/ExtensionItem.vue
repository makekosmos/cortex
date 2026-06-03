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

<style scoped>
.ext-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
}

.ext-icon {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  object-fit: cover;
  flex-shrink: 0;
}

.ext-icon-fallback {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1rem;
  font-weight: 600;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
}

.ext-info {
  flex: 1;
  min-width: 0;
}

.ext-name {
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--foreground);
}

.ext-meta {
  font-size: 0.6875rem;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  margin-top: 1px;
  display: flex;
  flex-wrap: wrap;
  column-gap: 4px;
  align-items: baseline;
}

.ext-description {
  font-size: 0.6875rem;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  margin-top: 3px;
}

.ext-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
</style>
