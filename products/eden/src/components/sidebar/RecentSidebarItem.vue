<template>
  <button
    type="button"
    class="eden-recent-sidebar-item"
    :class="{ 'is-active': active }"
    :data-testid="testId"
    :aria-current="active ? 'page' : undefined"
    @click="emit('click')"
    @contextmenu="emit('contextmenu', $event)"
  >
    <span class="eden-recent-sidebar-item__icon" aria-hidden="true">
      <component :is="icon" :size="16" :stroke-width="2" />
    </span>
    <span class="eden-recent-sidebar-item__body">
      <span class="eden-recent-sidebar-item__head">
        <span class="eden-recent-sidebar-item__title">{{ label }}</span>
      </span>
      <span v-if="meta" class="eden-recent-sidebar-item__meta">{{ meta }}</span>
    </span>
  </button>
</template>

<script setup lang="ts">
import type { Component } from "vue";

defineProps<{
  icon: Component;
  label: string;
  meta?: string;
  active?: boolean;
  testId?: string;
}>();

const emit = defineEmits<{
  click: [];
  contextmenu: [event: MouseEvent];
}>();
</script>

<style scoped>
.eden-recent-sidebar-item {
  position: relative;
  display: flex;
  height: 56px;
  width: 100%;
  min-width: 0;
  align-items: center;
  gap: 0.5rem;
  border: 0;
  border-radius: 0.375rem;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 78%, transparent);
  cursor: default;
  padding: 0.375rem 0.5rem;
  text-align: left;
}

.eden-recent-sidebar-item::after {
  content: "";
  position: absolute;
  right: 0.5rem;
  bottom: 0;
  left: 2.5rem;
  height: 1px;
  pointer-events: none;
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
}

.eden-recent-sidebar-item:hover {
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
}

.eden-recent-sidebar-item.is-active,
.eden-recent-sidebar-item.is-active:hover {
  background: color-mix(in srgb, var(--accent) 15%, transparent);
  color: var(--foreground);
}

.eden-recent-sidebar-item.is-active::after {
  opacity: 0;
}

.eden-recent-sidebar-item__icon {
  display: inline-flex;
  width: 1.5rem;
  height: 1.5rem;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
}

.eden-recent-sidebar-item__body {
  display: grid;
  min-width: 0;
  flex: 1 1 auto;
  gap: 0.0625rem;
}

.eden-recent-sidebar-item__head {
  display: grid;
  min-width: 0;
  grid-template-columns: minmax(0, 1fr);
  align-items: center;
  gap: 0.25rem;
}

.eden-recent-sidebar-item__title,
.eden-recent-sidebar-item__meta {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.eden-recent-sidebar-item__title {
  font-family: var(--font-sans);
  font-size: 13px;
  font-weight: 600;
  line-height: 1.25;
}

.eden-recent-sidebar-item__meta {
  color: color-mix(in srgb, var(--foreground) 48%, transparent);
  font-family: var(--font-sans);
  font-size: 11px;
  font-weight: 500;
  line-height: 1.2;
}
</style>
