<script setup lang="ts">
defineProps<{
  label: string;
  color: string;
  active?: boolean;
}>();
defineEmits<{
  (e: "click"): void;
}>();
</script>

<template>
  <button type="button" class="item" :class="{ active }" @click="$emit('click')">
    <span class="dot" :style="{ background: color }"></span>
    <span class="label">{{ label }}</span>
  </button>
</template>

<style scoped>
.item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  width: 100%;
  height: 28px;
  border: none;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
  font-size: 13px;
  cursor: pointer;
  border-radius: 6px;
  text-align: left;
}

.item:hover {
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
}

.item.active {
  background: var(--surface);
  color: var(--foreground);
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
